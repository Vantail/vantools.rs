//! Generic filesystem checks shared by the commands.
//!
//! Every check inspects a path without following symbolic links, so a
//! command never accepts a link in place of the file or directory it
//! validated.

use std::fs;
use std::io;
use std::path::Path;

use crate::common::Error;
use crate::common::validation::escape_path;

/// Return an error unless `path` is an existing directory.
///
/// A symlink at the path is refused instead of followed, so the source
/// directory obeys the same no-symlink policy as its `manifest.json`,
/// `dist/`, and the archive path.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] for a symlink and
/// [`Error::Runtime`] when the path is missing, is not a directory, or
/// cannot be inspected.
pub fn require_directory(path: &Path, description: &str) -> Result<(), Error> {
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
pub fn refuse_symlink(path: &Path) -> Result<(), Error> {
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

/// Return an error unless `path` is an existing regular file.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the path is missing or is not a
/// regular file.
pub fn require_file(path: &Path, description: &str) -> Result<(), Error> {
    if path.is_file() {
        return Ok(());
    }
    return Err(Error::Runtime(format!(
        "{description} not found: {}",
        escape_path(path)
    )));
}
