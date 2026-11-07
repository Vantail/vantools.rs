//! Archive inspection and validation for the `verify` command.
//!
//! The module owns every check the command runs on an archive: the
//! archive path itself must be a regular file, every entry name must
//! pass the shared archive rules and be unique, every entry must be a
//! regular file or an explicit directory entry (`dist/`), every entry is
//! read through so its data and CRC-32 are
//! validated within the decompressed-size budgets, the archived
//! `manifest.json` is validated with the rules the `pack` command
//! applies, and the `dist/` directory and its entrypoint must exist.
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

use super::VerifyReport;

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

/// Open the archive at `source` and run every check on it.
///
/// The checks run in the order the report prints them, and the first
/// failure ends the run, so an invalid archive is never partly
/// reported.
///
/// # Errors
///
/// Returns the first validation or runtime error the checks produce.
pub(super) fn inspect(source: &Path) -> Result<VerifyReport, Error> {
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
    let mut archive = match opened {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to read archive {}: {error}",
                escape_path(source)
            )));
        }
    };
    let (names, manifest_index) = collect_entry_names(&mut archive, source)?;
    let (manifest, manifest_bytes) =
        read_manifest(&mut archive, manifest_index, source)?;
    require_dist(&names, source)?;
    require_entrypoint(&names, &manifest)?;
    return Ok(VerifyReport {
        source: source.to_path_buf(),
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
/// during installation.
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
        if !seen.insert(name.clone()) {
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
/// The byte-level check reports only that duplicates are present, not
/// which name repeats, so both checks share this diagnostic, which
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
