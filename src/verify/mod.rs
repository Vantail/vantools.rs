//! Implementation of the `verify` command.
//!
//! The command opens an existing `.van` archive and validates it
//! without extracting anything: every entry name is checked with the
//! shared archive rules, every entry is read through so its data and
//! CRC-32 are validated, symbolic links are rejected, the archived
//! `manifest.json` is validated with the rules the `pack` command
//! applies, and the `dist/` directory and its entrypoint must exist.
//! A valid archive is reported with its manifest and its contents as a
//! tree.
//!
//! The archive path itself must be a regular file. A symbolic link, a
//! FIFO, or a device is refused before it is opened, because opening a
//! FIFO blocks until a writer appears and a device can block or return
//! unbounded data.

use std::cmp::Ordering;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};

use zip::ZipArchive;
use zip::read::ZipFile;

use crate::common::Error;
use crate::common::cli::CliCommand;
use crate::common::command::Command;
use crate::common::manifest::{MAX_MANIFEST_BYTES, Manifest};
use crate::common::output::{write_line, write_text};
use crate::common::validation::{
    escape_manifest, escape_path, escape_text, validate_archive_entry,
    validate_no_node_modules,
};
use crate::tui::progress::show_progress;

mod cli;

pub use cli::run_cli;

/// Mask that selects the file-type bits of a Unix mode.
const FILE_TYPE_MASK: u32 = 0o170000;

/// File-type bits of a symbolic link in a Unix mode.
const SYMLINK_FILE_TYPE: u32 = 0o120000;

/// File-type bits of a regular file in a Unix mode.
const REGULAR_FILE_TYPE: u32 = 0o100000;

/// Size of the buffer used to read one archive entry.
const COPY_BUFFER_BYTES: usize = 8 * 1024;

/// Bytes in one mebibyte, used to render the budgets in diagnostics.
const MEBIBYTE: u64 = 1024 * 1024;

/// Largest decompressed size accepted for a single archive entry, in
/// bytes.
///
/// The entry data is read in full, so an entry that expands to an
/// unbounded size would let a crafted archive exhaust memory or CPU.
/// The limit stays far above any plausible plugin payload.
const MAX_ENTRY_BYTES: u64 = 64 * MEBIBYTE;

/// Largest total decompressed size accepted for one archive, in bytes.
///
/// Every entry of the archive is read, so the total bounds the work a
/// single `verify` run can be made to do.
const MAX_TOTAL_BYTES: u64 = 256 * MEBIBYTE;

/// Largest number of entries accepted in one archive.
///
/// Every entry name is validated and kept for the report, so the count
/// bounds the memory a crafted central directory can make `verify`
/// allocate. The limit stays far above any plausible plugin payload.
const MAX_ENTRY_COUNT: usize = 4096;

/// Name of the manifest file expected at the archive root.
const MANIFEST_FILE_NAME: &str = "manifest.json";

/// Name of the directory that holds the plugin payload.
const DIST_DIRECTORY_NAME: &str = "dist";

/// The `verify` command.
///
/// Opens the archive at `source` and validates its structure without
/// extracting it: every entry name must be a safe relative path, no
/// entry may carry a `node_modules` component, every entry must be a
/// regular file, every entry must decompress to bytes that match its
/// CRC-32 within the decompressed-size budgets, the archive must hold
/// no more than the entry-count budget, the manifest must pass the
/// same checks `pack` applies, and `dist/` must contain the manifest
/// entrypoint. The archive path itself must be a regular file, so a
/// symbolic link, a FIFO, or a device is refused instead of being
/// opened. A valid archive is reported with its manifest and its
/// contents as a tree.
#[non_exhaustive]
pub struct VerifyCommand {
    /// The `.van` archive to verify; must be a regular file.
    pub source: PathBuf,
}

/// Everything a successful archive check produced.
struct ArchiveCheck {
    /// Entry names as the archive listed them; the report sorts them in
    /// place.
    names: Vec<String>,
    /// The parsed and validated manifest.
    manifest: Manifest,
    /// The exact manifest bytes that passed validation.
    manifest_bytes: Vec<u8>,
}

/// One node of the archive tree.
struct TreeNode {
    /// Name of the path segment this node represents.
    name: String,
    /// Children of this node, in sorted order.
    children: Vec<TreeNode>,
}

/// One directory that is open while the tree is built.
struct OpenDirectory {
    /// Name of the path segment this directory represents.
    name: String,
    /// Position of this directory in its parent's children.
    index: usize,
}

/// A node waiting to be rendered.
struct PendingNode<'a> {
    /// The node to render.
    node: &'a TreeNode,
    /// The prefix shared by every line below this node.
    prefix: String,
    /// Whether this node is the last child of its parent.
    is_last: bool,
}

impl Command for VerifyCommand {
    fn run(&self) -> Result<(), Error> {
        show_progress();
        let mut check = self.inspect()?;
        return print_report(&self.source, &mut check);
    }
}

impl VerifyCommand {
    /// Creates a `verify` command for the given archive.
    pub fn new(source: PathBuf) -> Self {
        return Self {
            source,
        };
    }

    /// Open the archive and run every check on it.
    ///
    /// The checks run in the order the report prints them, and the
    /// first failure ends the run, so an invalid archive is never
    /// partly reported.
    fn inspect(&self) -> Result<ArchiveCheck, Error> {
        require_regular_source(&self.source)?;
        let file = match File::open(&self.source) {
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to open archive {}: {error}",
                    escape_path(&self.source)
                )));
            }
        };
        let opened = ZipArchive::new(file);
        let mut archive = match opened {
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read archive {}: {error}",
                    escape_path(&self.source)
                )));
            }
        };
        let names = self.collect_entry_names(&mut archive)?;
        let (manifest, manifest_bytes) =
            self.read_manifest(&mut archive, &names)?;
        require_dist(&names, &self.source)?;
        require_entrypoint(&names, &manifest)?;
        return Ok(ArchiveCheck {
            names,
            manifest,
            manifest_bytes,
        });
    }

    /// Validate every entry name, read the entry data, and collect the
    /// entry names.
    ///
    /// Reading each entry to the end makes the ZIP library check the
    /// local header, the compressed stream, and its CRC-32, so an
    /// archive with corrupt or truncated data is rejected here instead
    /// of failing later during installation.
    ///
    /// The entry count is checked before any entry is read, so an
    /// archive holding an excessive number of entries is rejected
    /// without allocating a name per entry.
    fn collect_entry_names(
        &self,
        archive: &mut ZipArchive<File>,
    ) -> Result<Vec<String>, Error> {
        let count = archive.len();
        if count > MAX_ENTRY_COUNT {
            return Err(too_many_entries(&self.source));
        }
        let mut names: Vec<String> = Vec::new();
        let mut total_bytes: u64 = 0;
        for index in 0..count {
            let entry = archive.by_index(index);
            let mut entry = match entry {
                Ok(value) => value,
                Err(error) => {
                    return Err(Error::Runtime(format!(
                        "failed to read entry {index} of {}: {error}",
                        escape_path(&self.source)
                    )));
                }
            };
            let name = match entry.name() {
                Ok(value) => value.into_owned(),
                Err(error) => {
                    return Err(Error::Runtime(format!(
                        "failed to read the name of entry {index} of {}: {error}",
                        escape_path(&self.source)
                    )));
                }
            };
            validate_archive_entry(&name)?;
            validate_no_node_modules(Path::new(&name))?;
            reject_non_regular(&name, entry.unix_mode())?;
            read_entry_data(&mut entry, &name, &self.source, &mut total_bytes)?;
            names.push(name);
        }
        return Ok(names);
    }

    /// Read the archived manifest and validate its bytes.
    fn read_manifest(
        &self,
        archive: &mut ZipArchive<File>,
        names: &[String],
    ) -> Result<(Manifest, Vec<u8>), Error> {
        if !entry_present(names, MANIFEST_FILE_NAME) {
            return Err(Error::ValidationError(format!(
                "Missing 'manifest.json' in archive: {}",
                escape_path(&self.source)
            )));
        }
        let entry = match archive.by_name(MANIFEST_FILE_NAME) {
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read '{MANIFEST_FILE_NAME}' from {}: {error}",
                    escape_path(&self.source)
                )));
            }
        };
        let mut limited = entry.take(MAX_MANIFEST_BYTES + 1);
        let mut bytes: Vec<u8> = Vec::new();
        let read = limited.read_to_end(&mut bytes);
        match read {
            Ok(_) => {}
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read '{MANIFEST_FILE_NAME}' from {}: {error}",
                    escape_path(&self.source)
                )));
            }
        }
        let manifest = Manifest::from_bytes(&bytes)?;
        return Ok((manifest, bytes));
    }
}

/// Return an error unless the archive path is a regular file.
///
/// The path is inspected with [`fs::symlink_metadata`], so a symbolic
/// link is seen as a link instead of being followed. The check runs
/// before the path is opened, because a FIFO blocks the open until a
/// writer appears and a device can block or return unbounded data.
/// A missing path is left to [`File::open`], which reports it as a
/// failed open like any other unreadable archive.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] for a symbolic link and for any
/// other non-regular file type, and [`Error::Runtime`] when the path
/// cannot be inspected.
fn require_regular_source(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path);
    let metadata = match metadata {
        Ok(value) => value,
        Err(error) => {
            if error.kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                escape_path(path)
            )));
        }
    };
    if metadata.file_type().is_file() {
        return Ok(());
    }
    return Err(Error::ValidationError(format!(
        "Symlinks or non-regular files are not allowed: {}",
        escape_path(path)
    )));
}

/// Read one entry to the end so its data and checksum are validated.
///
/// `total_bytes` accumulates the decompressed size of every entry read
/// so far. The declared size is checked first, so an oversized entry is
/// rejected before it is decompressed, and the streamed bytes are
/// counted again because a header may understate its own length. The
/// data is never held in memory as a whole.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when the entry exceeds a budget,
/// and when its data cannot be read or does not match its CRC-32.
fn read_entry_data(
    entry: &mut ZipFile<'_, File>,
    name: &str,
    source: &Path,
    total_bytes: &mut u64,
) -> Result<(), Error> {
    let declared = entry.size();
    if declared > MAX_ENTRY_BYTES {
        return Err(entry_too_large(name));
    }
    let remaining = MAX_TOTAL_BYTES.saturating_sub(*total_bytes);
    if declared > remaining {
        return Err(contents_too_large(source));
    }
    let mut buffer = [0u8; COPY_BUFFER_BYTES];
    loop {
        let read = entry.read(&mut buffer);
        let count = match read {
            Ok(0) => return Ok(()),
            Ok(value) => value,
            Err(error) => {
                return Err(Error::ValidationError(format!(
                    "Failed to read entry data for {} in {}: {error}",
                    escape_text(name),
                    escape_path(source)
                )));
            }
        };
        let read_bytes = count as u64;
        *total_bytes = total_bytes.saturating_add(read_bytes);
        if *total_bytes > MAX_TOTAL_BYTES {
            return Err(contents_too_large(source));
        }
    }
}

/// Build the error for an entry above the per-entry decompressed
/// budget.
fn entry_too_large(name: &str) -> Error {
    let limit = MAX_ENTRY_BYTES / MEBIBYTE;
    return Error::ValidationError(format!(
        "Archive entry is larger than the {limit} MiB limit: {}",
        escape_text(name)
    ));
}

/// Build the error for an archive above the total decompressed budget.
fn contents_too_large(source: &Path) -> Error {
    let limit = MAX_TOTAL_BYTES / MEBIBYTE;
    return Error::ValidationError(format!(
        "Archive contents are larger than the {limit} MiB limit: {}",
        escape_path(source)
    ));
}

/// Build the error for an archive above the entry-count budget.
fn too_many_entries(source: &Path) -> Error {
    return Error::ValidationError(format!(
        "Archive has more than {MAX_ENTRY_COUNT} entries: {}",
        escape_path(source)
    ));
}

impl CliCommand for VerifyCommand {
    fn name(&self) -> &'static str {
        return "verify";
    }

    fn usage(&self) -> &'static str {
        return "verify -s <source>";
    }

    fn help(&self) -> &'static str {
        return "Usage: verify -s <source>\n\nOptions:\n  -s, --source <path>  The .van archive to verify (a regular file)\n  -h, --help           Print this help message\n  -v, --version        Print version information";
    }
}

/// Return true when `names` holds an entry named `expected`.
fn entry_present(names: &[String], expected: &str) -> bool {
    for name in names {
        if name == expected {
            return true;
        }
    }
    return false;
}

/// Return an error unless the entry mode marks a regular file.
///
/// The mode comes from the ZIP external attributes and is absent for
/// archives written on platforms without Unix modes, so a missing mode
/// passes, as does a mode that carries no file-type bits: without a
/// declared type the entry is a regular file, which is what every other
/// rule requires. Only the file-type bits are inspected, so the
/// permission bits of a regular entry are left alone.
///
/// A symbolic link keeps its own diagnostic because it is the most
/// common non-regular entry. Every other type, such as a directory or a
/// device, is rejected as well, so an entry the archive format does not
/// permit cannot pass by satisfying a name check.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] for a symbolic link and for any
/// other non-regular file type.
fn reject_non_regular(name: &str, mode: Option<u32>) -> Result<(), Error> {
    let mode = match mode {
        Some(value) => value,
        None => return Ok(()),
    };
    let file_type = mode & FILE_TYPE_MASK;
    if file_type == SYMLINK_FILE_TYPE {
        return Err(Error::ValidationError(format!(
            "Symlinks are not allowed in the archive: {}",
            escape_text(name)
        )));
    }
    if file_type == 0 || file_type == REGULAR_FILE_TYPE {
        return Ok(());
    }
    return Err(Error::ValidationError(format!(
        "Non-regular files are not allowed in the archive: {}",
        escape_text(name)
    )));
}

/// Return an error unless some entry lives below `dist/`.
fn require_dist(names: &[String], source: &Path) -> Result<(), Error> {
    let prefix = format!("{DIST_DIRECTORY_NAME}/");
    for name in names {
        if name.starts_with(prefix.as_str()) {
            return Ok(());
        }
    }
    return Err(Error::ValidationError(format!(
        "Missing 'dist' directory in archive: {}",
        escape_path(source)
    )));
}

/// Return an error unless the manifest entrypoint exists under `dist/`.
///
/// Every entry was already required to be a regular file, so a name
/// match also proves the entrypoint is a regular file rather than a
/// directory or device entry.
fn require_entrypoint(
    names: &[String],
    manifest: &Manifest,
) -> Result<(), Error> {
    let expected = format!("{DIST_DIRECTORY_NAME}/{}", manifest.entrypoint);
    for name in names {
        if name == &expected {
            return Ok(());
        }
    }
    return Err(Error::ValidationError(format!(
        "Entrypoint file not found in dist: {}",
        escape_text(&expected)
    )));
}

/// Print the result of a successful verification.
///
/// The collected entry names are sorted in place, so the report never
/// holds a second copy of them.
fn print_report(source: &Path, check: &mut ArchiveCheck) -> Result<(), Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let header = format!("Verifying archive: {}", archive_label(source));
    write_line(&mut handle, &header, "output")?;
    write_line(
        &mut handle,
        "  [OK] Paths and ZIP structure are valid.",
        "output",
    )?;
    write_line(
        &mut handle,
        "  [OK] manifest.json is present and valid.",
        "output",
    )?;
    let entrypoint = escape_text(&check.manifest.entrypoint);
    let entrypoint_line =
        format!("  [OK] dist/ and entrypoint ({entrypoint}) are present.");
    write_line(&mut handle, &entrypoint_line, "output")?;
    write_line(&mut handle, "", "output")?;
    write_line(&mut handle, "--- manifest.json ---", "output")?;
    write_manifest(&mut handle, &check.manifest_bytes)?;
    write_line(&mut handle, "---------------------", "output")?;
    write_line(&mut handle, "", "output")?;
    write_line(&mut handle, "Archive Contents:", "output")?;
    check.names.sort_by(compare_paths);
    let tree = build_tree(&check.names);
    for line in render_tree(&tree) {
        write_line(&mut handle, &line, "output")?;
    }
    write_line(&mut handle, "", "output")?;
    return write_line(&mut handle, "Archive is valid.", "output");
}

/// Print the manifest bytes, ending with one newline.
///
/// The archived manifest is untrusted, so its control characters are
/// rendered as visible escapes before they reach the terminal: printing
/// the bytes verbatim would let a crafted manifest emit terminal escape
/// sequences, overwrite the line, or forge log lines in captured output.
/// Line breaks are kept so a formatted manifest stays readable, and a
/// printable manifest is rendered unchanged.
fn write_manifest<W>(handle: &mut W, bytes: &[u8]) -> Result<(), Error>
where
    W: Write,
{
    let text = String::from_utf8_lossy(bytes);
    let escaped = escape_manifest(&text);
    write_text(handle, &escaped, "output")?;
    if escaped.ends_with('\n') {
        return Ok(());
    }
    return write_text(handle, "\n", "output");
}

/// Render the archive file name used in the report header.
fn archive_label(source: &Path) -> String {
    let name = match source.file_name() {
        Some(value) => value,
        None => return escape_path(source),
    };
    return escape_path(Path::new(name));
}

/// Compare two entry names so a directory stays next to its contents.
///
/// A plain byte order separates an entry such as `dist` from the
/// entries below it, `dist/app.ts`, whenever a sibling such as
/// `dist-extra/other.ts` sorts between them, because `/` is `0x2F` and
/// so orders after most punctuation. Comparing `/` below every other
/// byte keeps each directory and everything below it contiguous, which
/// is what [`build_tree`] relies on to reuse an open directory instead
/// of creating a second node with the same name.
fn compare_paths(left: &String, right: &String) -> Ordering {
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    let limit = left_bytes.len().min(right_bytes.len());
    let mut position = 0;
    while position < limit {
        let left_byte = match left_bytes.get(position) {
            Some(value) => *value,
            None => break,
        };
        let right_byte = match right_bytes.get(position) {
            Some(value) => *value,
            None => break,
        };
        let left_key = path_order_key(left_byte);
        let right_key = path_order_key(right_byte);
        if left_key != right_key {
            return left_key.cmp(&right_key);
        }
        position += 1;
    }
    return left_bytes.len().cmp(&right_bytes.len());
}

/// Return the sort key of one byte of an entry name.
///
/// `/` maps below every other byte, so it sorts before any character
/// that could otherwise separate a directory from its contents, and
/// the remaining bytes keep their usual order.
fn path_order_key(byte: u8) -> u16 {
    if byte == b'/' {
        return 0;
    }
    return u16::from(byte) + 1;
}

/// Build the tree of path segments from the ordered entry names.
///
/// [`compare_paths`] keeps every entry contiguous with the entries
/// that share its directory, so one pass can carry the chain of open
/// directories and create only the segments an entry adds. No sibling
/// list is scanned, so a directory holding many entries costs work
/// proportional to the number of path segments instead of a scan per
/// entry.
fn build_tree(names: &[String]) -> TreeNode {
    let mut root = TreeNode {
        name: String::new(),
        children: Vec::new(),
    };
    let mut open: Vec<OpenDirectory> = Vec::new();
    for name in names {
        insert_sorted_path(&mut root, &mut open, name);
    }
    return root;
}

/// Insert one entry whose name belongs after the previous entry.
///
/// `open` holds the directory created for every segment of the
/// previously inserted entry. The segments this entry shares with it
/// are reused, and only the remaining segments are created.
fn insert_sorted_path(
    root: &mut TreeNode,
    open: &mut Vec<OpenDirectory>,
    path: &str,
) {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        segments.push(segment);
    }
    let shared = shared_segments(open, &segments);
    open.truncate(shared);
    let mut position = shared;
    while position < segments.len() {
        let segment = match segments.get(position) {
            Some(value) => value,
            None => return,
        };
        let parent = match node_at_mut(root, open) {
            Some(value) => value,
            None => return,
        };
        let name = String::from(*segment);
        parent.children.push(TreeNode {
            name: name.clone(),
            children: Vec::new(),
        });
        let index = parent.children.len() - 1;
        open.push(OpenDirectory {
            name,
            index,
        });
        position += 1;
    }
}

/// Return the directory reached by following `open` from `root`.
fn node_at_mut<'a>(
    root: &'a mut TreeNode,
    open: &[OpenDirectory],
) -> Option<&'a mut TreeNode> {
    let mut current = root;
    for directory in open {
        current = current.children.get_mut(directory.index)?;
    }
    return Some(current);
}

/// Count the leading segments `open` and `segments` share.
fn shared_segments(open: &[OpenDirectory], segments: &[&str]) -> usize {
    let limit = open.len().min(segments.len());
    let mut shared = 0;
    while shared < limit {
        let directory = match open.get(shared) {
            Some(value) => value,
            None => return shared,
        };
        let segment = match segments.get(shared) {
            Some(value) => value,
            None => return shared,
        };
        if directory.name.as_str() != *segment {
            return shared;
        }
        shared += 1;
    }
    return shared;
}

/// Render the tree below `root` as one line per node.
///
/// Pending nodes are kept on the heap, so the nesting depth of an
/// archive cannot exhaust the stack. A node with children is a
/// directory and is rendered with a trailing `/`.
fn render_tree(root: &TreeNode) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut pending: Vec<PendingNode> = Vec::new();
    queue_children(root, "", &mut pending);
    loop {
        let next = pending.pop();
        let current = match next {
            None => return lines,
            Some(value) => value,
        };
        let connector = match current.is_last {
            true => "└── ",
            false => "├── ",
        };
        let mut rendered = escape_path(Path::new(&current.node.name));
        if !current.node.children.is_empty() {
            rendered.push('/');
        }
        lines.push(format!("{}{connector}{rendered}", current.prefix));
        let child_prefix = format!(
            "{}{}",
            current.prefix,
            match current.is_last {
                true => "    ",
                false => "│   ",
            }
        );
        queue_children(current.node, &child_prefix, &mut pending);
    }
}

/// Queue the children of `node` so the first child is rendered first.
fn queue_children<'a>(
    node: &'a TreeNode,
    prefix: &str,
    pending: &mut Vec<PendingNode<'a>>,
) {
    let count = node.children.len();
    let mut position = count;
    for child in node.children.iter().rev() {
        let is_last = position == count;
        pending.push(PendingNode {
            node: child,
            prefix: String::from(prefix),
            is_last,
        });
        position -= 1;
    }
}
