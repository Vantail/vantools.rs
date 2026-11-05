//! Implementation of the `pack` command.
//!
//! This module holds the `PackCommand` type and the source validation it
//! drives. Argument parsing lives in the `cli` submodule and the
//! low-level archive writing lives in the `writer` submodule.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::common::Error;
use crate::common::cli::CliCommand;
use crate::common::command::Command;
use crate::common::manifest::Manifest;
use crate::common::output::write_line;
use crate::common::validation::{
    escape_path, validate_archive_entry, validate_file_type,
    validate_no_node_modules,
};
use crate::common::walk::{WalkEntry, walk_tree};

mod cli;
mod writer;

use writer::write_archive;

pub use cli::run_cli;

/// Name of the manifest file expected at the source root.
const MANIFEST_FILE_NAME: &str = "manifest.json";

/// Name of the directory that holds the plugin payload.
const DIST_DIRECTORY_NAME: &str = "dist";

/// Extension of the archives this command produces.
const ARCHIVE_EXTENSION: &str = "van";

/// The `pack` command.
///
/// Reads a plugin source directory and writes a `<name>-<version>.van`
/// archive into the target directory. An existing archive with the same
/// name is overwritten after a warning is printed to stderr; an existing
/// symlink or other non-regular entry is refused so the archive cannot
/// be redirected outside the target directory. A successful run then
/// reports the archive path, the files written, and the source files
/// the layout excluded.
///
/// The source and target directories themselves are inspected without
/// following links, so a symlinked directory is refused rather than
/// resolved, matching the no-symlink policy applied to every other path
/// the command touches.
///
/// The archive layout follows Section 4 of `docs/SPEC.md`: the
/// validated `manifest.json` sits at the archive root and only the
/// contents of `dist/` are added, each under the `dist/` prefix. Other
/// entries of the source directory are never read into the archive.
#[non_exhaustive]
pub struct PackCommand {
    /// The plugin source directory holding `manifest.json` and `dist/`.
    pub source: PathBuf,
    /// The directory that receives the generated archive.
    pub target: PathBuf,
}

/// A validated source file scheduled for the archive.
struct ArchiveEntry {
    /// Name of the entry inside the archive.
    archive_name: String,
    /// Path of the file on disk.
    source_path: PathBuf,
}

/// Everything a validated source directory contributes to the archive.
struct CollectedPack {
    /// The parsed and validated `manifest.json`.
    manifest: Manifest,
    /// The exact manifest bytes that passed validation.
    manifest_bytes: Vec<u8>,
    /// The files scheduled for the archive, in walk order.
    entries: Vec<ArchiveEntry>,
    /// The source files the archive layout excludes.
    ignored: Vec<PathBuf>,
}

impl Command for PackCommand {
    fn run(&self) -> Result<(), Error> {
        let dist_path = self.validate_layout()?;
        let collected = self.collect_pack(&dist_path)?;
        return self.write_collected(collected);
    }
}

impl PackCommand {
    /// Creates a `pack` command for the given source and target.
    pub fn new(source: PathBuf, target: PathBuf) -> Self {
        return Self {
            source,
            target,
        };
    }

    /// Build the archive path from the manifest name and version.
    fn archive_path(&self, manifest: &Manifest) -> PathBuf {
        let archive_name = format!(
            "{}-{}.{ARCHIVE_EXTENSION}",
            manifest.name, manifest.version
        );
        return self.target.join(archive_name);
    }

    /// Create the target directory when it does not exist yet.
    ///
    /// A symlink at the target path is refused instead of followed, so
    /// the archive cannot land in a directory other than the one the
    /// command named.
    fn ensure_target(&self) -> Result<(), Error> {
        refuse_symlink(&self.target)?;
        if self.target.is_dir() {
            return Ok(());
        }
        let created = fs::create_dir_all(&self.target);
        return match created {
            Ok(()) => notice(&format!(
                "created target directory: {}",
                self.target.display()
            )),
            Err(error) => Err(Error::Runtime(format!(
                "failed to create target directory {}: {error}",
                escape_path(&self.target)
            ))),
        };
    }

    /// Validate the source layout and return the `dist` directory.
    ///
    /// This covers the first phase of a pack run: the source directory,
    /// its `manifest.json`, and its `dist/` directory must all exist,
    /// each as a regular directory or file rather than a symlink. The
    /// checks run in this fixed order so the first failure is always the
    /// same error, with the same type and path, as before.
    fn validate_layout(&self) -> Result<PathBuf, Error> {
        let manifest_path = self.source.join(MANIFEST_FILE_NAME);
        let dist_path = self.source.join(DIST_DIRECTORY_NAME);
        require_directory(&self.source, "source directory")?;
        require_file(&manifest_path, MANIFEST_FILE_NAME)?;
        validate_file_type(&manifest_path)?;
        require_dist_directory(&self.source, &dist_path)?;
        validate_file_type(&dist_path)?;
        return Ok(dist_path);
    }

    /// Load the manifest and collect everything an archive will contain.
    ///
    /// The manifest is read and validated, the entrypoint is checked
    /// against `dist`, then the walk collects the archive entries and
    /// the ignored paths into a [`CollectedPack`].
    fn collect_pack(&self, dist_path: &Path) -> Result<CollectedPack, Error> {
        let loaded = Manifest::from_dir_with_bytes(&self.source)?;
        let (manifest, manifest_bytes) = loaded;
        require_entrypoint(dist_path, &manifest.entrypoint)?;
        let mut entries: Vec<ArchiveEntry> = Vec::new();
        collect_entries(dist_path, DIST_DIRECTORY_NAME, &mut entries)?;
        let mut ignored: Vec<PathBuf> = Vec::new();
        collect_ignored(&self.source, &mut ignored)?;
        return Ok(CollectedPack {
            manifest,
            manifest_bytes,
            entries,
            ignored,
        });
    }

    /// Prepare the target, write the archive, and report the result.
    ///
    /// This covers the final phase of a pack run: the target directory
    /// is created, an existing archive is warned about, the archive is
    /// written, and the ignored paths are sorted for the summary.
    fn write_collected(&self, collected: CollectedPack) -> Result<(), Error> {
        let CollectedPack {
            manifest,
            manifest_bytes,
            entries,
            ignored: mut source_ignored,
        } = collected;
        self.ensure_target()?;
        let archive_path = self.archive_path(&manifest);
        warn_if_overwriting(&archive_path)?;
        write_archive(&archive_path, &manifest_bytes, &entries)?;
        source_ignored.sort();
        let files_written = entries.len() + 1;
        return report_success(&archive_path, files_written, &source_ignored);
    }
}

impl CliCommand for PackCommand {
    fn name(&self) -> &'static str {
        return "pack";
    }

    fn usage(&self) -> &'static str {
        return "pack -s <source> -t <target>";
    }

    fn help(&self) -> &'static str {
        return "Usage: pack -s <source> -t <target>\n\nOptions:\n  -s, --source <path>  The source directory containing manifest.json and dist/\n  -t, --target <path>  The target directory to place the .van archive\n  -h, --help           Print this help message\n  -v, --version        Print version information";
    }
}

/// Return an error unless `path` is an existing directory.
///
/// A symlink at the path is refused instead of followed, so the source
/// directory obeys the same no-symlink policy as its `manifest.json`,
/// `dist/`, and the archive path.
fn require_directory(path: &Path, description: &str) -> Result<(), Error> {
    refuse_symlink(path)?;
    if path.is_dir() {
        return Ok(());
    }
    return Err(Error::Runtime(format!(
        "{description} not found: {}",
        escape_path(path)
    )));
}

/// Refuse `path` when it is a symbolic link instead of following it.
///
/// The path is inspected with [`fs::symlink_metadata`], so a symlink is
/// seen as a link whether or not its own target exists. A missing path
/// passes through, because each caller reports a missing path in its
/// own way. This mirrors the refusal `writer` applies to the archive
/// destination.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] for a symlink and
/// [`Error::Runtime`] when the path cannot be inspected.
fn refuse_symlink(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path);
    match metadata {
        Ok(value) => {
            if value.file_type().is_symlink() {
                return Err(Error::ValidationError(format!(
                    "Symlinks or non-regular files are not allowed: {}",
                    escape_path(path)
                )));
            }
            return Ok(());
        }
        Err(error) => {
            if error.kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                escape_path(path)
            )));
        }
    }
}

/// Return an error unless `<source>/dist` is an existing directory.
///
/// The reported path is the source directory, because a missing `dist/`
/// means the source is not a packable plugin rather than that the user
/// passed a wrong `dist` path.
fn require_dist_directory(
    source: &Path,
    dist_path: &Path,
) -> Result<(), Error> {
    if dist_path.is_dir() {
        return Ok(());
    }
    return Err(Error::ValidationError(format!(
        "Missing 'dist' directory in source: {}",
        escape_path(source)
    )));
}

/// Return an error unless `<dist>/<entrypoint>` is an existing file.
///
/// The `entrypoint` is validated as a relative archive path when the
/// manifest is read, so joining it onto `dist_path` cannot escape the
/// `dist` directory.
///
/// A symlink at the entrypoint path is refused instead of followed, so
/// the dedicated check enforces the no-symlink policy itself instead of
/// relying on the later `dist/` walk. The link is rejected with the
/// same diagnostic [`validate_file_type`] uses for `dist/` and its
/// contents. A missing or non-regular path, such as a directory or a
/// FIFO, is reported as missing because the entrypoint must be a
/// regular file.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] for a symlinked, non-regular, or
/// missing entrypoint, and [`Error::Runtime`] when the path cannot be
/// inspected.
fn require_entrypoint(dist_path: &Path, entrypoint: &str) -> Result<(), Error> {
    let entrypoint_path = dist_path.join(entrypoint);
    refuse_symlink(&entrypoint_path)?;
    if entrypoint_path.is_file() {
        return Ok(());
    }
    return Err(Error::ValidationError(format!(
        "Entrypoint file not found in dist: {}",
        escape_path(&entrypoint_path)
    )));
}

/// Return an error unless `path` is an existing regular file.
fn require_file(path: &Path, description: &str) -> Result<(), Error> {
    if path.is_file() {
        return Ok(());
    }
    return Err(Error::Runtime(format!(
        "{description} not found: {}",
        escape_path(path)
    )));
}

/// Print a notice about a successful side effect to stdout.
fn notice(message: &str) -> Result<(), Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    return write_line(&mut handle, message, "output");
}

/// Warn on stderr when the archive path already exists.
///
/// The warning goes to stderr so that stdout keeps carrying only the
/// progress notices of a successful pack.
fn warn_if_overwriting(path: &Path) -> Result<(), Error> {
    if !path.exists() {
        return Ok(());
    }
    let message =
        format!("Warning: Overwriting existing archive: {}", escape_path(path));
    return warn(&message);
}

/// Print a warning to stderr.
fn warn(message: &str) -> Result<(), Error> {
    let stderr = io::stderr();
    let mut handle = stderr.lock();
    return write_line(&mut handle, message, "warning");
}

/// Print the summary of a completed pack to stdout.
///
/// The summary names the archive, counts the files written, and lists
/// the source files the layout excluded. Every interpolated path is
/// escaped like any other diagnostic.
fn report_success(
    archive_path: &Path,
    files_written: usize,
    ignored: &[PathBuf],
) -> Result<(), Error> {
    let mut message = format!(
        "Successfully created archive: {}\nFiles written: {}",
        escape_path(archive_path),
        files_written
    );
    let ignored_count = ignored.len();
    message.push_str(&format!("\nFiles ignored: {ignored_count}"));
    if !ignored.is_empty() {
        message.push_str("\nIgnored paths:");
        for relative in ignored {
            let rendered = escape_path(relative);
            message.push_str(&format!("\n  - {rendered}"));
        }
    }
    return notice(&message);
}

/// Collect the files of `source` that the archive layout excludes.
///
/// The manifest at the source root and the `dist/` directory belong to
/// the archive, so every other file is reported to the user.
fn collect_ignored(
    source: &Path,
    ignored: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    let mut visit = |entry: &WalkEntry| -> Result<bool, Error> {
        if entry.file_type.is_dir() {
            let relative = entry.relative.as_path();
            return Ok(relative != Path::new(DIST_DIRECTORY_NAME));
        }
        if entry.relative.as_path() == Path::new(MANIFEST_FILE_NAME) {
            return Ok(false);
        }
        ignored.push(entry.relative.clone());
        return Ok(false);
    };
    return walk_tree(source, &mut visit);
}

/// Validate and collect every file below `directory` under `prefix`.
///
/// The walk itself lives in [`walk_tree`], which keeps pending
/// directories on the heap, so the nesting depth of the source tree
/// cannot exhaust the stack.
fn collect_entries(
    directory: &Path,
    prefix: &str,
    entries: &mut Vec<ArchiveEntry>,
) -> Result<(), Error> {
    let mut visit = |entry: &WalkEntry| -> Result<bool, Error> {
        validate_file_type(&entry.path)?;
        let archive_name = archive_name_of(prefix, &entry.relative)?;
        validate_no_node_modules(Path::new(&archive_name))?;
        validate_archive_entry(&archive_name)?;
        if entry.file_type.is_dir() {
            return Ok(true);
        }
        entries.push(ArchiveEntry {
            archive_name,
            source_path: entry.path.clone(),
        });
        return Ok(false);
    };
    return walk_tree(directory, &mut visit);
}

/// Build an archive name from a prefix and a path relative to it.
///
/// Archive names always use `/`, and `validate_archive_entry` rejects
/// any backslash, so a name never depends on the host separator.
fn archive_name_of(prefix: &str, relative: &Path) -> Result<String, Error> {
    let mut name = String::from(prefix);
    for component in relative.components() {
        let part = match component.as_os_str().to_str() {
            Some(value) => value,
            None => {
                return Err(Error::Runtime(format!(
                    "file name is not valid Unicode: {}",
                    escape_path(relative)
                )));
            }
        };
        name.push('/');
        name.push_str(part);
    }
    return Ok(name);
}
