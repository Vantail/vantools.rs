//! Shared archive inspection for the `verify` and `unpack` commands.
//!
//! The module owns every check both commands run on an archive: the
//! archive path itself must be a regular file, every entry name must
//! pass the shared archive rules and be unique ignoring case, every
//! entry must be a regular file or an explicit directory entry
//! (`dist/`), every entry is
//! read through so its data and CRC-32 are
//! validated within the decompressed-size budgets, the archived
//! `manifest.json` is validated with the rules the `pack` command
//! applies, and the `dist/` directory and its entrypoint must exist.
//!
//! Keeping one implementation means `unpack` accepts exactly the
//! archives `verify` accepts, so no archive can pass one command and be
//! rejected by the other.
//!
//! The archive path is checked before it is opened, because opening a
//! FIFO blocks until a writer appears and a device can block or return
//! unbounded data.

use std::collections::BTreeSet;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;

use zip::ZipArchive;
use zip::read::ZipFile;

use crate::common::Error;
use crate::common::manifest::{MAX_MANIFEST_BYTES, Manifest};
use crate::common::validation::{
    escape_path, escape_text, validate_archive_entry, validate_no_node_modules,
};

/// Mask that selects the file-type bits of a Unix mode.
const FILE_TYPE_MASK: u32 = 0o170000;

/// File-type bits of a symbolic link in a Unix mode.
const SYMLINK_FILE_TYPE: u32 = 0o120000;

/// File-type bits of a regular file in a Unix mode.
const REGULAR_FILE_TYPE: u32 = 0o100000;

/// File-type bits of a directory in a Unix mode.
const DIRECTORY_FILE_TYPE: u32 = 0o040000;

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
/// single run can be made to do.
const MAX_TOTAL_BYTES: u64 = 256 * MEBIBYTE;

/// Largest number of entries accepted in one archive.
///
/// Every entry name is validated and kept for the report, so the count
/// bounds the memory a crafted central directory can make a run
/// allocate. The limit stays far above any plausible plugin payload.
const MAX_ENTRY_COUNT: usize = 4096;

/// Largest archive entry name accepted, in bytes.
///
/// The inspection keeps every name in the collected list and in the
/// duplicate sets, and case-folding can expand it, so the count cap
/// alone would let 4096 maximum-length ZIP names (up to 65,535 bytes
/// each) allocate hundreds of megabytes before any data budget applies.
/// 255 bytes matches the budget the manifest applies to a single file
/// name component and bounds the bookkeeping together with
/// [`MAX_ENTRY_COUNT`].
const MAX_ENTRY_NAME_BYTES: usize = 255;

/// Name of the manifest file expected at the archive root.
const MANIFEST_FILE_NAME: &str = "manifest.json";

/// Name of the directory that holds the plugin payload.
const DIST_DIRECTORY_NAME: &str = "dist";

/// Everything a successful archive inspection produced.
///
/// The commands built on this result decide on their own what to report
/// and what to write; the inspection itself only validates and collects.
#[non_exhaustive]
pub struct ArchiveContents {
    /// The parsed and validated manifest.
    pub manifest: Manifest,
    /// The exact manifest bytes that passed validation.
    pub manifest_bytes: Vec<u8>,
    /// Entry names as the archive listed them, in archive order.
    pub entry_names: Vec<String>,
}

/// Open the archive at `source` and run every check on it.
///
/// The checks run in the order the `verify` report prints them, and the
/// first failure ends the run, so an invalid archive is never partly
/// reported and nothing downstream sees a half-validated result.
///
/// The archive is opened once and inspected through that handle.
/// Callers that also extract entries should use [`open`] and
/// [`inspect_archive`] instead, so the extraction reads exactly the
/// handle that was validated.
///
/// # Errors
///
/// Returns the first validation or runtime error the checks produce.
pub fn inspect(source: &Path) -> Result<ArchiveContents, Error> {
    let mut opened = open(source)?;
    return inspect_archive(&mut opened, source);
}

/// Open the archive at `source` for reading.
///
/// The path must be a regular file, so a symbolic link, a FIFO, or a
/// device is refused before it is opened. The handle must be inspected
/// with [`inspect_archive`] before any entry is used, and a caller that
/// also extracts entries must extract through this same handle: a path
/// replaced after inspection could otherwise supply different bytes to
/// the extraction than the ones that were validated.
///
/// # Errors
///
/// Returns the errors of the path check, and [`Error::Runtime`] when
/// the archive cannot be opened or parsed.
pub(crate) fn open(source: &Path) -> Result<ZipArchive<File>, Error> {
    require_regular_source(source)?;
    let file = match File::open(source) {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to open archive {}: {error}",
                escape_path(source)
            )));
        }
    };
    let opened = ZipArchive::new(file);
    return match opened {
        Ok(value) => Ok(value),
        Err(error) => Err(Error::Runtime(format!(
            "failed to read archive {}: {error}",
            escape_path(source)
        ))),
    };
}

/// Run every check on an archive that was already opened.
///
/// The checks are the ones [`inspect`] runs: entry names, ZIP structure,
/// the decompressed-size budgets, the manifest, and the `dist/`
/// directory with its entrypoint. The first failure ends the run.
///
/// # Errors
///
/// Returns the first validation or runtime error the checks produce.
pub(crate) fn inspect_archive(
    archive: &mut ZipArchive<File>,
    source: &Path,
) -> Result<ArchiveContents, Error> {
    let (names, manifest_index) = collect_entry_names(archive, source)?;
    let (manifest, manifest_bytes) =
        read_manifest(archive, manifest_index, source)?;
    require_dist(&names, source)?;
    require_entrypoint(&names, &manifest)?;
    return Ok(ArchiveContents {
        manifest,
        manifest_bytes,
        entry_names: names,
    });
}

/// Validate every entry name, read the entry data, and collect the
/// entry names.
///
/// Reading each entry to the end makes the ZIP library check the local
/// header, the compressed stream, and its CRC-32, so an archive with
/// corrupt or truncated data is rejected here instead of failing later
/// during extraction.
///
/// The entry count is checked before any entry is read, so an archive
/// holding an excessive number of entries is rejected without
/// allocating a name per entry.
///
/// Entry names must be unique, as Section 6 of `docs/SPEC.md` requires.
/// Byte-identical duplicates are rejected before any entry is read, and
/// every decoded name is checked against the names already seen,
/// because the reader decodes a name written in another encoding to the
/// same string: an entry stored as CP437 and one stored as UTF-8 can
/// report one name while keeping different bytes. Either way, the
/// payload this run validates could differ from the payload another
/// consumer extracts under that name.
///
/// Names that differ only by case are rejected as well, because an
/// extractor on Windows or macOS writes both of them to one file. See
/// [`folded_name`].
///
/// The returned index is the position of the `manifest.json` entry, so
/// the manifest is later read from exactly that entry.
fn collect_entry_names(
    archive: &mut ZipArchive<File>,
    source: &Path,
) -> Result<(Vec<String>, Option<usize>), Error> {
    let count = archive.len();
    if count > MAX_ENTRY_COUNT {
        return Err(too_many_entries(source));
    }
    if archive.has_duplicate_names() {
        return Err(duplicate_entry(source));
    }
    let mut names: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut folded: BTreeSet<String> = BTreeSet::new();
    let mut manifest_index: Option<usize> = None;
    let mut total_bytes: u64 = 0;
    for index in 0..count {
        let entry = archive.by_index(index);
        let mut entry = match entry {
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read entry {index} of {}: {error}",
                    escape_path(source)
                )));
            }
        };
        let name = match entry.name() {
            Ok(value) => value.into_owned(),
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read the name of entry {index} of {}: {error}",
                    escape_path(source)
                )));
            }
        };
        if name.len() > MAX_ENTRY_NAME_BYTES {
            return Err(Error::ValidationError(format!(
                "Archive entry name is longer than {MAX_ENTRY_NAME_BYTES} bytes: {}",
                escape_text(&name)
            )));
        }
        if !seen.insert(name.clone()) {
            return Err(duplicate_entry(source));
        }
        if !folded.insert(folded_name(&name)) {
            return Err(duplicate_entry(source));
        }
        validate_archive_entry(&name)?;
        validate_no_node_modules(Path::new(&name))?;
        if name == MANIFEST_FILE_NAME {
            manifest_index = Some(index);
        }
        reject_non_regular(&name, entry.unix_mode())?;
        read_entry_data(&mut entry, &name, source, &mut total_bytes)?;
        names.push(name);
    }
    return Ok((names, manifest_index));
}

/// Return the key used to detect entry names that collide on a file
/// system that compares names without regard to case.
///
/// Windows (NTFS) and macOS (APFS) resolve `dist/app.ts` and
/// `dist/App.ts` to one file, while this archive keeps both, so the
/// payload this run validates can differ from the payload an extractor
/// writes. The key uses the Unicode uppercase mapping, the direction
/// those file systems fold in, so the ASCII collisions and the common
/// Unicode ones are caught. It approximates their tables rather than
/// copying them, which is why Section 6 of `docs/SPEC.md` states the
/// rule as a portability requirement.
fn folded_name(name: &str) -> String {
    return name.to_uppercase();
}

/// Read the archived manifest and validate its bytes.
///
/// The entry is selected by the index [`collect_entry_names`] recorded
/// rather than by name, so the bytes validated here are the bytes of
/// the entry whose name passed the uniqueness and path checks. A
/// name-based lookup could resolve to a different entry than the one
/// that was validated.
fn read_manifest(
    archive: &mut ZipArchive<File>,
    manifest_index: Option<usize>,
    source: &Path,
) -> Result<(Manifest, Vec<u8>), Error> {
    let index = match manifest_index {
        Some(value) => value,
        None => {
            return Err(Error::ValidationError(format!(
                "Missing 'manifest.json' in archive: {}",
                escape_path(source)
            )));
        }
    };
    let entry = match archive.by_index(index) {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to read '{MANIFEST_FILE_NAME}' from {}: {error}",
                escape_path(source)
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
                escape_path(source)
            )));
        }
    }
    let manifest = Manifest::from_bytes(&bytes)?;
    return Ok((manifest, bytes));
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

/// Build the error for an archive that lists one entry name twice.
///
/// The byte-level check, and the case-folded check that catches names
/// differing only by case, report only that a collision is present and
/// not which name collides, so the checks share this diagnostic, which
/// names the archive instead.
fn duplicate_entry(source: &Path) -> Error {
    return Error::ValidationError(format!(
        "Duplicate archive entry names are not allowed: {}",
        escape_path(source)
    ));
}

/// Return an error unless the entry mode marks a regular file or an
/// explicit directory entry.
///
/// The mode comes from the ZIP external attributes and is absent for
/// archives written on platforms without Unix modes, so a missing mode
/// passes, as does a mode that carries no file-type bits: without a
/// declared type the entry is a regular file, which is what every other
/// rule requires. Only the file-type bits are inspected, so the
/// permission bits of a regular entry are left alone.
///
/// A directory mode is accepted only for a name that ends with `/`, which
/// is how a standard ZIP archive writes a directory entry. A directory
/// mode on a file name stays rejected, because such an entry would
/// shadow a real file below `dist/`.
///
/// A symbolic link keeps its own diagnostic because it is the most
/// common non-regular entry. Every other type, such as a device, is
/// rejected as well, so an entry the archive format does not permit
/// cannot pass by satisfying a name check.
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
    if file_type == DIRECTORY_FILE_TYPE && name.ends_with('/') {
        return Ok(());
    }
    return Err(Error::ValidationError(format!(
        "Non-regular files are not allowed in the archive: {}",
        escape_text(name)
    )));
}

/// Return an error unless some entry lives below `dist/`.
///
/// An explicit directory entry (`dist/`) proves the directory exists just
/// as a file entry (`dist/app.ts`) does.
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
/// The name must match exactly, and directory entries are skipped, so
/// an entry that only differs from the expected name by a trailing
/// slash does not satisfy the check. The manifest validation rejects a
/// trailing slash in the entrypoint itself, so the expected name always
/// names a file, and a `dist/app.ts/` directory entry can never stand
/// in for an `app.ts` entrypoint that carries no payload.
fn require_entrypoint(
    names: &[String],
    manifest: &Manifest,
) -> Result<(), Error> {
    let expected = format!("{DIST_DIRECTORY_NAME}/{}", manifest.entrypoint);
    for name in names {
        if name.ends_with('/') {
            continue;
        }
        if name == &expected {
            return Ok(());
        }
    }
    return Err(Error::ValidationError(format!(
        "Entrypoint file not found in dist: {}",
        escape_text(&expected)
    )));
}
