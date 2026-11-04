//! Reusable path validation rules for `.van` plugin archives.
//!
//! The checks implement Sections 3 and 6 of `docs/SPEC.md` so that every
//! command that reads or writes archives can share one implementation.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use unicode_normalization::is_nfc;

use crate::common::Error;

/// Directory name that archives must not contain at any depth.
const FORBIDDEN_DIRECTORY: &str = "node_modules";

/// Validates a raw relative archive entry path.
///
/// Rejects backslashes, absolute paths, drive letters, empty path
/// segments, `.` and `..` traversal segments, and paths that are not
/// normalized to Unicode NFC.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] describing the first rule the
/// path violates.
pub fn validate_archive_entry(path_str: &str) -> Result<(), Error> {
    if path_str.contains('\\') {
        return Err(Error::ValidationError(format!(
            "Backslashes are not allowed in paths: {path_str}"
        )));
    }
    if is_absolute_or_has_empty_segment(path_str) {
        return Err(Error::ValidationError(format!(
            "Absolute paths or drive letters are not allowed: {path_str}"
        )));
    }
    if has_traversal_segment(path_str) {
        return Err(Error::ValidationError(format!(
            "Path traversal segments are not allowed: {path_str}"
        )));
    }
    if !is_nfc(path_str) {
        return Err(Error::ValidationError(format!(
            "Path is not in Unicode NFC format: {path_str}"
        )));
    }
    return Ok(());
}

/// Validates that `path` is a regular file or a directory.
///
/// The path is inspected with [`fs::symlink_metadata`], so symbolic
/// links are reported as links instead of being followed.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the metadata cannot be read, and
/// [`Error::ValidationError`] for symbolic links and any other
/// non-regular file.
pub fn validate_file_type(path: &Path) -> Result<(), Error> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                path.display()
            )));
        }
    };
    let file_type = metadata.file_type();
    if file_type.is_file() || file_type.is_dir() {
        return Ok(());
    }
    return Err(symlink_error(path));
}

/// Validates that no component of `path` is named `node_modules`.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when a `node_modules` component
/// is present.
pub fn validate_no_node_modules(path: &Path) -> Result<(), Error> {
    for component in path.components() {
        if component.as_os_str() == OsStr::new(FORBIDDEN_DIRECTORY) {
            return Err(Error::ValidationError(format!(
                "Forbidden directory '{FORBIDDEN_DIRECTORY}' found at: {}",
                path.display()
            )));
        }
    }
    return Ok(());
}

/// Returns true when the path is absolute, has a drive letter, or has
/// an empty segment.
fn is_absolute_or_has_empty_segment(path_str: &str) -> bool {
    if path_str.starts_with('/') {
        return true;
    }
    if path_str.contains(':') {
        return true;
    }
    for segment in path_str.split('/') {
        if segment.is_empty() {
            return true;
        }
    }
    return false;
}

/// Returns true when the path has a `.` or `..` segment.
fn has_traversal_segment(path_str: &str) -> bool {
    for segment in path_str.split('/') {
        if segment == "." || segment == ".." {
            return true;
        }
    }
    return false;
}

/// Build the error for a symbolic link or other non-regular file.
fn symlink_error(path: &Path) -> Error {
    return Error::ValidationError(format!(
        "Symlinks or non-regular files are not allowed: {}",
        path.display()
    ));
}
