//! Low-level archive writing for the `pack` command.
//!
//! Every archive is built in a temporary sibling file and moved over
//! its final path with one rename, so a failed run never leaves a
//! truncated archive at the destination.

use std::fs::{self, File};
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};

use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::common::Error;
use crate::common::plugin::MANIFEST_FILE_NAME;
use crate::common::validation::{
    escape_path, escape_text, open_validated_file,
};

use super::ArchiveEntry;

/// Size of the buffer used to copy one source file into the archive.
const COPY_BUFFER_BYTES: usize = 8 * 1024;

/// Write the archive into a temporary sibling file, then rename it over
/// the archive path.
///
/// A failed run would otherwise truncate an existing archive at
/// `<name>-<version>.van` and leave a corrupt, non-ZIP file on disk:
/// the destination is now only replaced by one rename of a fully
/// written, finished temporary archive, and every failure deletes the
/// temporary file instead. The finished archive is flushed to disk
/// before the rename, so a successful run survives an unclean shutdown
/// instead of leaving a truncated file behind.
pub(super) fn write_archive(
    archive_path: &Path,
    manifest_bytes: &[u8],
    entries: &[ArchiveEntry],
) -> Result<(), Error> {
    refuse_non_regular_archive(archive_path)?;
    let temp_path = temp_archive_path(archive_path)?;
    let file = create_temp_file(&temp_path)?;
    let built = build_archive(&temp_path, file, manifest_bytes, entries);
    return match built {
        Ok(()) => install_archive(&temp_path, archive_path),
        Err(error) => {
            discard_temp_file(&temp_path);
            return Err(error);
        }
    };
}

/// Derive the sibling temporary path for the archive being written.
///
/// The temporary file lives in the same directory as the archive so
/// that the final rename stays on one volume and replaces the archive
/// in one step. The process id keeps the name unique per running pack
/// process, including two concurrent runs of the same command.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the archive path has no file name.
fn temp_archive_path(archive_path: &Path) -> Result<PathBuf, Error> {
    let name = match archive_path.file_name() {
        Some(value) => value,
        None => {
            return Err(Error::Runtime(format!(
                "failed to derive a temporary archive path for {}",
                escape_path(archive_path)
            )));
        }
    };
    let mut temp_name = name.to_owned();
    temp_name.push(format!(".tmp-{}", std::process::id()));
    return Ok(archive_path.with_file_name(temp_name));
}

/// Refuse to replace a symlink or other non-regular entry at `path`.
///
/// The entry at the archive path is inspected with
/// [`fs::symlink_metadata`], so a symlink is seen as a link and never
/// followed or truncated by the archive write.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] for any existing non-regular
/// entry and [`Error::Runtime`] when the path cannot be inspected.
fn refuse_non_regular_archive(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path);
    match metadata {
        Ok(value) => {
            if !value.file_type().is_file() {
                return Err(Error::ValidationError(format!(
                    "Refusing to overwrite a symlink or non-regular file: {}",
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

/// Create the temporary archive file without following an entry.
///
/// The file is opened with `create_new`, which fails when the path
/// already exists, so an entry replaced during the check-then-create
/// window can never be followed or truncated. A pre-existing entry is
/// refused rather than removed: it may belong to the user instead of an
/// earlier run, and `unpack` treats its temporary directory the same
/// way.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when an entry already exists at
/// the path, and [`Error::Runtime`] when the file cannot be created.
fn create_temp_file(path: &Path) -> Result<File, Error> {
    let opened = open_new_file(path);
    return match opened {
        Ok(value) => Ok(value),
        Err(error) => {
            if error.kind() == io::ErrorKind::AlreadyExists {
                return Err(Error::ValidationError(format!(
                    "Temporary archive already exists: {}",
                    escape_path(path)
                )));
            }
            Err(Error::Runtime(format!(
                "failed to create {}: {error}",
                escape_path(path)
            )))
        }
    };
}

/// Open `path` for writing only when it does not exist yet.
fn open_new_file(path: &Path) -> io::Result<File> {
    let opened = fs::OpenOptions::new().write(true).create_new(true).open(path);
    return opened;
}

/// Options applied to every archive entry: `deflate` compression.
fn default_file_options() -> SimpleFileOptions {
    return SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated);
}

/// Write the manifest and the collected entries into `file`, finish the
/// ZIP stream, and flush the result to disk.
///
/// The finished archive is synced with [`File::sync_all`] before the
/// caller renames it into place, so an unclean shutdown cannot leave a
/// zero-length or partially written file where a successful `pack`
/// reported an archive.
///
/// # Errors
///
/// Returns any error of the archive writes or of the final sync,
/// reported against the temporary path.
fn build_archive(
    temp_path: &Path,
    file: File,
    manifest_bytes: &[u8],
    entries: &[ArchiveEntry],
) -> Result<(), Error> {
    let mut writer = ZipWriter::new(file);
    let options = default_file_options();
    add_manifest(&mut writer, manifest_bytes, options)?;
    write_entries(&mut writer, entries, options)?;
    let finished = writer.finish();
    let finished_file = match finished {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to finish {}: {error}",
                escape_path(temp_path)
            )));
        }
    };
    let synced = finished_file.sync_all();
    return match synced {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to sync {}: {error}",
            escape_path(temp_path)
        ))),
    };
}

/// Rename the finished temporary archive over its final path.
///
/// The rename replaces an existing regular archive in one step, so the
/// destination never holds a truncated archive while a run is writing.
/// When the temporary path was swapped for a non-regular entry, or the
/// rename fails, the temporary path is removed so the target directory
/// keeps no debris and an existing archive stays untouched.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when the temporary path was
/// meanwhile swapped for a symlink or other non-regular entry, and
/// [`Error::Runtime`] when the rename fails.
fn install_archive(temp_path: &Path, archive_path: &Path) -> Result<(), Error> {
    // The temporary path is inside the target directory and thus
    // writable by whoever can write the archive. An entry swapped in
    // during the write-to-rename window is refused rather than moved
    // over the archive path, mirroring the refusal at that path.
    let checked = refuse_non_regular_archive(temp_path);
    return match checked {
        Ok(()) => rename_over(temp_path, archive_path),
        Err(error) => {
            discard_temp_file(temp_path);
            return Err(error);
        }
    };
}

/// Rename the checked temporary archive over its final path.
///
/// On failure the temporary file is removed so a failed run never
/// leaves temporary debris behind.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the rename fails.
fn rename_over(temp_path: &Path, archive_path: &Path) -> Result<(), Error> {
    let renamed = fs::rename(temp_path, archive_path);
    return match renamed {
        Ok(()) => Ok(()),
        Err(error) => {
            discard_temp_file(temp_path);
            return Err(Error::Runtime(format!(
                "failed to move {} to {}: {error}",
                escape_path(temp_path),
                escape_path(archive_path)
            )));
        }
    };
}

/// Delete the temporary archive after a failed write.
///
/// Removal is best-effort so that the original error is the one
/// reported to the user.
fn discard_temp_file(path: &Path) {
    // Best effort; the original error is reported either way.
    let _ = fs::remove_file(path);
}

/// Add the validated manifest bytes to the archive root.
fn add_manifest(
    writer: &mut ZipWriter<File>,
    manifest_bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<(), Error> {
    return add_file(writer, MANIFEST_FILE_NAME, manifest_bytes, options);
}

/// Start one archive entry under `archive_name`.
fn start_entry(
    writer: &mut ZipWriter<File>,
    archive_name: &str,
    options: SimpleFileOptions,
) -> Result<(), Error> {
    let started = writer.start_file(archive_name, options);
    return match started {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to add {}: {error}",
            escape_text(archive_name)
        ))),
    };
}

/// Add `content` to the archive under `archive_name`.
fn add_file(
    writer: &mut ZipWriter<File>,
    archive_name: &str,
    content: &[u8],
    options: SimpleFileOptions,
) -> Result<(), Error> {
    start_entry(writer, archive_name, options)?;
    let written = writer.write_all(content);
    return match written {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to write {}: {error}",
            escape_text(archive_name)
        ))),
    };
}

/// Write the collected entries into the archive.
fn write_entries(
    writer: &mut ZipWriter<File>,
    entries: &[ArchiveEntry],
    options: SimpleFileOptions,
) -> Result<(), Error> {
    for entry in entries {
        add_source_file(writer, entry, options)?;
    }
    return Ok(());
}

/// Open one collected file and add it to the archive.
///
/// The file is opened through [`open_validated_file`], so a path swapped
/// for a symlink or another file after collection is rejected instead
/// of contributing content from elsewhere. The content is streamed into
/// the archive instead of being held in memory.
fn add_source_file(
    writer: &mut ZipWriter<File>,
    entry: &ArchiveEntry,
    options: SimpleFileOptions,
) -> Result<(), Error> {
    let entry_path = &entry.source_path;
    let mut file = open_validated_file(entry_path)?;
    start_entry(writer, &entry.archive_name, options)?;
    return copy_stream(writer, &mut file, &entry.archive_name, entry_path);
}

/// Copy `reader` into the current archive entry in fixed-size chunks.
///
/// The content is never held in memory as a whole, so a large source
/// file cannot exhaust memory while it is packed.
fn copy_stream(
    writer: &mut ZipWriter<File>,
    reader: &mut File,
    archive_name: &str,
    source_path: &Path,
) -> Result<(), Error> {
    let mut buffer = [0u8; COPY_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer);
        let count = match read {
            Ok(0) => return Ok(()),
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read {}: {error}",
                    escape_path(source_path)
                )));
            }
        };
        let chunk = match buffer.get(..count) {
            Some(value) => value,
            None => {
                return Err(Error::Runtime(format!(
                    "failed to copy {}: chunk exceeds the buffer",
                    escape_path(source_path)
                )));
            }
        };
        let written = writer.write_all(chunk);
        match written {
            Ok(()) => {}
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to write {}: {error}",
                    escape_text(archive_name)
                )));
            }
        }
    }
}
