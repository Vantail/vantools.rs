//! Low-level extraction for the `unpack` command.
//!
//! Every file is written into a temporary sibling directory and the
//! finished directory is moved to its final path with one rename, so a
//! failed run never leaves a half-extracted directory at the
//! destination. The archive has already passed the shared inspection, so
//! entry names are safe relative paths and every entry is a regular
//! file; the writer still refuses an existing destination so a run can
//! never overwrite files the user already had.
//!
//! The destination is the `<name>-<version>` directory the command built
//! inside the requested target, so the temporary directory is created
//! next to it in that target.

use std::fs::{self, File};
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};

use zip::ZipArchive;
use zip::read::ZipFile;

use crate::common::Error;
use crate::common::validation::escape_path;

/// Size of the buffer used to copy one archive entry to disk.
const COPY_BUFFER_BYTES: usize = 8 * 1024;

/// Name of the manifest file expected at the archive root.
const MANIFEST_FILE_NAME: &str = "manifest.json";

/// Extract every entry of `archive_path` into `destination_path` and
/// return the number of files written.
///
/// The destination is the `<name>-<version>` directory inside the
/// requested target. The layout reproduces the source layout `pack`
/// archives, so the manifest lands at `<destination>/manifest.json` and
/// the payload keeps its `dist/` prefix, for example
/// `<destination>/dist/app.ts`.
///
/// The manifest is written from the bytes that passed validation rather
/// than re-serialized from the parsed struct, so the extracted file is
/// byte-identical to the archived one.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when the destination already
/// exists, and [`Error::Runtime`] when the archive cannot be read or the
/// files cannot be written. Every failure removes the temporary
/// directory, so a failed run leaves neither a destination directory nor
/// temporary debris.
pub(super) fn extract_archive(
    archive_path: &Path,
    destination_path: &Path,
    manifest_bytes: &[u8],
) -> Result<usize, Error> {
    refuse_existing_target(destination_path)?;
    let temp_path = temp_directory_path(destination_path)?;
    let prepared = prepare_temp_directory(&temp_path);
    let extracted = match prepared {
        Ok(()) => extract_into(&temp_path, archive_path, manifest_bytes),
        Err(error) => Err(error),
    };
    return match extracted {
        Ok(written) => install_directory(&temp_path, destination_path, written),
        Err(error) => {
            discard_temp_directory(&temp_path);
            return Err(error);
        }
    };
}

/// Return an error when anything already occupies the destination path.
///
/// The path is inspected with [`fs::symlink_metadata`], so a dangling
/// symbolic link is seen as an entry instead of being treated as
/// missing. The destination never exists during a normal run, so every
/// existing entry is refused rather than replaced.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when the path exists, and
/// [`Error::Runtime`] when it cannot be inspected.
fn refuse_existing_target(destination_path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(destination_path);
    return match metadata {
        Ok(_) => Err(Error::ValidationError(format!(
            "Target directory already exists: {}",
            escape_path(destination_path)
        ))),
        Err(error) => {
            if error.kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                escape_path(destination_path)
            )));
        }
    };
}

/// Derive the temporary directory that sits next to the destination.
///
/// The temporary directory lives in the requested target, beside the
/// `<name>-<version>` destination, so the final rename stays on one
/// volume and installs the directory in one step. The process id keeps
/// the name unique per running unpack process.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the destination path has no file
/// name.
fn temp_directory_path(destination_path: &Path) -> Result<PathBuf, Error> {
    let name = match destination_path.file_name() {
        Some(value) => value,
        None => {
            return Err(Error::Runtime(format!(
                "failed to derive a temporary directory path for {}",
                escape_path(destination_path)
            )));
        }
    };
    let mut temp_name = name.to_owned();
    temp_name.push(format!(".tmp-{}", std::process::id()));
    return Ok(destination_path.with_file_name(temp_name));
}

/// Remove a leftover temporary entry and create the temporary
/// directory.
///
/// A `.tmp-` sibling left by an aborted run is removed first, so a retry
/// never mixes freshly extracted files with stale ones from the earlier
/// attempt. Missing parent directories are created, matching the way
/// `pack` creates a target directory.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the leftover cannot be removed or the
/// directory cannot be created.
fn prepare_temp_directory(path: &Path) -> Result<(), Error> {
    clear_leftover(path)?;
    let created = fs::create_dir_all(path);
    return match created {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to create {}: {error}",
            escape_path(path)
        ))),
    };
}

/// Remove an entry at `path` when one exists, whatever its type.
///
/// The entry is inspected without following links, so a leftover
/// symlink is removed as a link instead of having its own directory
/// emptied.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the path cannot be inspected or the
/// entry cannot be removed.
fn clear_leftover(path: &Path) -> Result<(), Error> {
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
    let file_type = metadata.file_type();
    let removed = match file_type.is_dir() && !file_type.is_symlink() {
        true => fs::remove_dir_all(path),
        false => fs::remove_file(path),
    };
    return match removed {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to remove {}: {error}",
            escape_path(path)
        ))),
    };
}

/// Open the archive and write its contents into `temp_path`.
///
/// The manifest is written first, then every regular entry of the
/// archive except the manifest itself, so the extracted tree keeps the
/// names the archive listed. Explicit directory entries are skipped,
/// because the parent directories of the files they describe are
/// created on demand.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the archive cannot be read or an
/// entry cannot be written to disk.
fn extract_into(
    temp_path: &Path,
    archive_path: &Path,
    manifest_bytes: &[u8],
) -> Result<usize, Error> {
    let opened = File::open(archive_path);
    let file = match opened {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to open archive {}: {error}",
                escape_path(archive_path)
            )));
        }
    };
    let read = ZipArchive::new(file);
    let mut archive = match read {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to read archive {}: {error}",
                escape_path(archive_path)
            )));
        }
    };
    let manifest_path = temp_path.join(MANIFEST_FILE_NAME);
    write_file(&manifest_path, manifest_bytes)?;
    let written = write_entries(&mut archive, temp_path, archive_path)?;
    return Ok(written + 1);
}

/// Write every regular entry of the archive except the manifest.
fn write_entries(
    archive: &mut ZipArchive<File>,
    temp_path: &Path,
    archive_path: &Path,
) -> Result<usize, Error> {
    let count = archive.len();
    let mut written: usize = 0;
    for index in 0..count {
        let entry = archive.by_index(index);
        let mut entry = match entry {
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read entry {index} of {}: {error}",
                    escape_path(archive_path)
                )));
            }
        };
        let name = match entry.name() {
            Ok(value) => value.into_owned(),
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read the name of entry {index} of {}: {error}",
                    escape_path(archive_path)
                )));
            }
        };
        if name == MANIFEST_FILE_NAME || name.ends_with('/') {
            continue;
        }
        write_entry(&mut entry, temp_path, archive_path, &name)?;
        written += 1;
    }
    return Ok(written);
}

/// Write one archive entry to `<temp_path>/<name>`.
///
/// The entry name passed the shared inspection, so it is a relative path
/// without traversal, drive letters, or backslashes, and joining it onto
/// the temporary directory keeps the write inside that directory. The
/// content is copied in fixed-size chunks, so a large entry is never
/// held in memory as a whole.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when a parent directory or the file cannot
/// be created, and when the entry cannot be read or written.
fn write_entry(
    entry: &mut ZipFile<'_, File>,
    temp_path: &Path,
    archive_path: &Path,
    name: &str,
) -> Result<(), Error> {
    let path = temp_path.join(name);
    create_parent_directory(&path)?;
    let mut file = create_file(&path)?;
    let mut buffer = [0u8; COPY_BUFFER_BYTES];
    loop {
        let read = entry.read(&mut buffer);
        let count = match read {
            Ok(0) => return Ok(()),
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read entry data for {} from {}: {error}",
                    escape_path(&path),
                    escape_path(archive_path)
                )));
            }
        };
        let chunk = match buffer.get(..count) {
            Some(value) => value,
            None => {
                return Err(Error::Runtime(format!(
                    "failed to copy {}: chunk exceeds the buffer",
                    escape_path(&path)
                )));
            }
        };
        let written = file.write_all(chunk);
        match written {
            Ok(()) => {}
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to write {}: {error}",
                    escape_path(&path)
                )));
            }
        }
    }
}

/// Write `content` to `path`, creating missing parent directories.
fn write_file(path: &Path, content: &[u8]) -> Result<(), Error> {
    create_parent_directory(path)?;
    let mut file = create_file(path)?;
    let written = file.write_all(content);
    return match written {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to write {}: {error}",
            escape_path(path)
        ))),
    };
}

/// Create the parent directory of `path` when there is one.
fn create_parent_directory(path: &Path) -> Result<(), Error> {
    let parent = match path.parent() {
        Some(value) => value,
        None => return Ok(()),
    };
    let created = fs::create_dir_all(parent);
    return match created {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to create {}: {error}",
            escape_path(parent)
        ))),
    };
}

/// Create the file at `path` for writing.
fn create_file(path: &Path) -> Result<File, Error> {
    let created = File::create(path);
    return match created {
        Ok(value) => Ok(value),
        Err(error) => Err(Error::Runtime(format!(
            "failed to create {}: {error}",
            escape_path(path)
        ))),
    };
}

/// Rename the finished temporary directory onto the destination path.
///
/// On failure the temporary directory is removed, so a failed run never
/// leaves temporary debris behind.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the rename fails.
fn install_directory(
    temp_path: &Path,
    destination_path: &Path,
    written: usize,
) -> Result<usize, Error> {
    let renamed = fs::rename(temp_path, destination_path);
    return match renamed {
        Ok(()) => Ok(written),
        Err(error) => {
            discard_temp_directory(temp_path);
            return Err(Error::Runtime(format!(
                "failed to move {} to {}: {error}",
                escape_path(temp_path),
                escape_path(destination_path)
            )));
        }
    };
}

/// Delete the temporary directory after a failed run.
///
/// Removal is best-effort so that the original error is the one
/// reported to the user.
fn discard_temp_directory(path: &Path) {
    // Best effort; the original error is reported either way.
    let _ = fs::remove_dir_all(path);
}
