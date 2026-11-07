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
//!
//! Every extracted file is synced to disk, and the finished temporary
//! directory is flushed before the rename, so a reported success cannot
//! leave truncated files behind after a crash.

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

/// Extract every entry of an inspected archive into `destination_path`
/// and return the number of files written.
///
/// `archive` must be the handle the inspection validated, so the bytes
/// that are written are the bytes that passed it: a path replaced in the
/// meantime cannot change the payload. `archive_path` is used only to
/// name the archive in diagnostics.
///
/// The destination is the `<name>-<version>` directory inside the
/// requested target. The layout reproduces the source layout `pack`
/// archives, so the manifest lands at `<destination>/manifest.json` and
/// the payload keeps its `dist/` prefix, for example
/// `<destination>/dist/app.ts`.
///
/// The manifest is written from the bytes that passed validation rather
/// than re-serialized from the parsed struct, so the extracted file is
/// byte-identical to the archived one. Every file is synced before the
/// directory is renamed, and the temporary directory itself is flushed
/// first, matching the durability `pack` applies to its archive.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when the destination or the
/// temporary path already exists, and [`Error::Runtime`] when an entry
/// cannot be read or the files cannot be written. Every failure after
/// the temporary directory was created removes it, so a failed run
/// leaves neither a destination directory nor temporary debris; an
/// entry that already occupied the temporary path is left untouched.
pub(super) fn extract_archive(
    archive: &mut ZipArchive<File>,
    archive_path: &Path,
    destination_path: &Path,
    manifest_bytes: &[u8],
) -> Result<usize, Error> {
    refuse_existing_target(destination_path)?;
    let temp_path = temp_directory_path(destination_path)?;
    let prepared = prepare_temp_directory(&temp_path);
    match prepared {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let extracted =
        extract_into(archive, &temp_path, archive_path, manifest_bytes);
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
/// the name unique per running unpack process; because the name is
/// predictable, [`prepare_temp_directory`] creates it with create-new
/// semantics and refuses any entry that already exists there.
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

/// Create the temporary directory with create-new semantics.
///
/// The path is not cleared first: an entry that already exists there,
/// including a symbolic link, fails the run so a pre-placed entry is
/// never removed or reused. The created entry is inspected without
/// following links and must be a real directory before anything is
/// written into it.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when an entry already exists at
/// the path, and [`Error::Runtime`] when the directory cannot be
/// created or inspected.
fn prepare_temp_directory(path: &Path) -> Result<(), Error> {
    let created = fs::create_dir(path);
    match created {
        Ok(()) => {}
        Err(error) => {
            if error.kind() == io::ErrorKind::AlreadyExists {
                return Err(Error::ValidationError(format!(
                    "Temporary directory already exists: {}",
                    escape_path(path)
                )));
            }
            return Err(Error::Runtime(format!(
                "failed to create temporary directory {}: {error}",
                escape_path(path)
            )));
        }
    }
    let metadata = fs::symlink_metadata(path);
    let metadata = match metadata {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                escape_path(path)
            )));
        }
    };
    let file_type = metadata.file_type();
    if file_type.is_dir() && !file_type.is_symlink() {
        return Ok(());
    }
    return Err(Error::Runtime(format!(
        "temporary directory is not a real directory: {}",
        escape_path(path)
    )));
}

/// Write the inspected archive's contents into `temp_path`.
///
/// The manifest is written first, then every regular entry of the
/// archive except the manifest itself, so the extracted tree keeps the
/// names the archive listed. Explicit directory entries are skipped,
/// because the parent directories of the files they describe are
/// created on demand.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when an entry cannot be read or written to
/// disk.
fn extract_into(
    archive: &mut ZipArchive<File>,
    temp_path: &Path,
    archive_path: &Path,
    manifest_bytes: &[u8],
) -> Result<usize, Error> {
    let manifest_path = temp_path.join(MANIFEST_FILE_NAME);
    write_file(&manifest_path, manifest_bytes)?;
    let written = write_entries(archive, temp_path, archive_path)?;
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
/// held in memory as a whole. The finished file is synced before the
/// function returns, so a crash cannot leave it truncated after success.
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
            Ok(0) => break,
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
    return sync_file(&file, &path);
}

/// Write `content` to `path`, creating missing parent directories, and
/// flush the file to disk before returning.
fn write_file(path: &Path, content: &[u8]) -> Result<(), Error> {
    create_parent_directory(path)?;
    let mut file = create_file(path)?;
    let written = file.write_all(content);
    match written {
        Ok(()) => {}
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to write {}: {error}",
                escape_path(path)
            )));
        }
    }
    return sync_file(&file, path);
}

/// Flush a written file's contents to disk.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the file cannot be synced.
fn sync_file(file: &File, path: &Path) -> Result<(), Error> {
    let synced = file.sync_all();
    return match synced {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to sync {}: {error}",
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

/// Create a file for writing without reusing an existing entry.
///
/// `create_new` fails when the path already exists, so a file or symlink
/// placed at the path is refused instead of being truncated or followed.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the file cannot be created.
fn create_file(path: &Path) -> Result<File, Error> {
    let opened = fs::OpenOptions::new().write(true).create_new(true).open(path);
    return match opened {
        Ok(value) => Ok(value),
        Err(error) => Err(Error::Runtime(format!(
            "failed to create {}: {error}",
            escape_path(path)
        ))),
    };
}

/// Flush the temporary directory to disk before the rename.
///
/// The directory is opened and synced, so the names created inside it
/// survive an unclean shutdown. `std` cannot open a directory on
/// non-Unix platforms, so those platforms rely on the per-file syncs.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the directory cannot be opened or
/// synced.
#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), Error> {
    let opened = File::open(path);
    let directory = match opened {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to open {}: {error}",
                escape_path(path)
            )));
        }
    };
    let synced = directory.sync_all();
    return match synced {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to sync {}: {error}",
            escape_path(path)
        ))),
    };
}

/// Flush the temporary directory to disk before the rename.
///
/// `std` cannot open a directory on this platform, so the file contents
/// were already synced and the directory is left to the file system.
///
/// # Errors
///
/// This implementation cannot fail.
#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), Error> {
    return Ok(());
}

/// Rename the finished temporary directory onto the destination path.
///
/// The directory is synced before the rename, and on failure the
/// temporary directory is removed, so a failed run never leaves
/// temporary debris behind.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the directory cannot be synced or the
/// rename fails.
fn install_directory(
    temp_path: &Path,
    destination_path: &Path,
    written: usize,
) -> Result<usize, Error> {
    let synced = sync_directory(temp_path);
    match synced {
        Ok(()) => {}
        Err(error) => {
            discard_temp_directory(temp_path);
            return Err(error);
        }
    }
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

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    use tempfile::tempdir;
    use zip::CompressionMethod;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use super::extract_archive;
    use crate::common::archive;

    /// Manifest used by the archives in these tests.
    const MANIFEST: &str = "{\"name\":\"sample-plugin\",\"version\":\"0.1.0\",\"entrypoint\":\"app.ts\"}";

    #[test]
    fn extraction_reads_the_handle_that_was_validated() {
        let root = match tempdir() {
            Ok(value) => value,
            Err(error) => {
                panic!("failed to create a temporary directory: {error}");
            }
        };
        let archive_path = root.path().join("sample.van");
        let entries: [(&str, &str); 2] =
            [("manifest.json", MANIFEST), ("dist/app.ts", "// validated\n")];
        write_archive(&archive_path, &entries);
        let mut handle = match archive::open(&archive_path) {
            Ok(value) => value,
            Err(error) => panic!("failed to open the archive: {error}"),
        };
        let inspected =
            match archive::inspect_archive(&mut handle, &archive_path) {
                Ok(value) => value,
                Err(error) => panic!("the valid archive was rejected: {error}"),
            };
        // Replace the path with an archive that would fail validation.
        let replacement = root.path().join("replacement.van");
        let swapped: [(&str, &str); 3] = [
            ("manifest.json", MANIFEST),
            ("dist/app.ts", "// swapped\n"),
            ("dist/../escaped.ts", "// escaped\n"),
        ];
        write_archive(&replacement, &swapped);
        let replaced = fs::rename(&replacement, &archive_path);
        match replaced {
            Ok(()) => {}
            Err(error) => panic!("failed to replace the archive: {error}"),
        }
        let target = root.path().join("out");
        let created = fs::create_dir(&target);
        match created {
            Ok(()) => {}
            Err(error) => {
                panic!("failed to create {}: {error}", target.display());
            }
        }
        let destination = target.join("sample-plugin-0.1.0");
        let extracted = extract_archive(
            &mut handle,
            &archive_path,
            &destination,
            &inspected.manifest_bytes,
        );
        let written = match extracted {
            Ok(value) => value,
            Err(error) => panic!("extraction failed: {error}"),
        };
        assert_eq!(written, 2);
        assert_file_content(
            &destination.join("dist").join("app.ts"),
            "// validated\n",
        );
        assert!(!destination.join("escaped.ts").exists());
    }

    /// Write an archive holding `entries` in the given order.
    fn write_archive(path: &Path, entries: &[(&str, &str)]) {
        let created = fs::File::create(path);
        let file = match created {
            Ok(value) => value,
            Err(error) => {
                panic!("failed to create {}: {error}", path.display());
            }
        };
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated);
        for (name, content) in entries {
            let started = writer.start_file(*name, options);
            match started {
                Ok(()) => {}
                Err(error) => panic!("failed to add {name}: {error}"),
            }
            let written = writer.write_all(content.as_bytes());
            match written {
                Ok(()) => {}
                Err(error) => panic!("failed to write {name}: {error}"),
            }
        }
        let finished = writer.finish();
        match finished {
            Ok(_) => {}
            Err(error) => {
                panic!("failed to finish {}: {error}", path.display());
            }
        }
        return;
    }

    /// Read a file and compare it with the expected text.
    fn assert_file_content(path: &Path, expected: &str) {
        let content = fs::read(path);
        let bytes = match content {
            Ok(value) => value,
            Err(error) => {
                panic!("failed to read {}: {error}", path.display());
            }
        };
        assert_eq!(bytes.as_slice(), expected.as_bytes());
    }
}
