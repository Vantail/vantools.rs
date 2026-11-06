//! Reusable path validation rules for `.van` plugin archives.
//!
//! The checks implement Sections 3 and 6 of `docs/SPEC.md` so that every
//! command that reads or writes archives can share one implementation.
//!
//! This module also renders untrusted names and paths for diagnostics, so
//! every command reports a bad file name the same safe way.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use unicode_normalization::is_nfc;

use crate::common::Error;

/// Directory name that archives must not contain at any depth.
const FORBIDDEN_DIRECTORY: &str = "node_modules";

/// Largest number of characters of an untrusted value echoed in a
/// diagnostic.
///
/// Validation and CLI errors interpolate untrusted values, such as a raw
/// command line argument or a manifest `name` member, into their text.
/// Capping the rendered value keeps an oversized but legal value from
/// flooding the terminal or CI log with printable garbage, matching the
/// bound the `van` dispatcher already applies. Truncated values are
/// marked with [`ELLIPSIS`].
pub const MAX_ECHO_CHARS: usize = 64;

/// Marker appended to a rendered untrusted value that was truncated.
const ELLIPSIS: &str = "...";

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
            "Backslashes are not allowed in paths: {}",
            escape_text(path_str)
        )));
    }
    if is_absolute_or_has_empty_segment(path_str) {
        return Err(Error::ValidationError(format!(
            "Absolute paths or drive letters are not allowed: {}",
            escape_text(path_str)
        )));
    }
    if has_traversal_segment(path_str) {
        return Err(Error::ValidationError(format!(
            "Path traversal segments are not allowed: {}",
            escape_text(path_str)
        )));
    }
    if !is_nfc(path_str) {
        return Err(Error::ValidationError(format!(
            "Path is not in Unicode NFC format: {}",
            escape_text(path_str)
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
    let validated = validated_metadata(path);
    return match validated {
        Ok(_) => Ok(()),
        Err(error) => Err(error),
    };
}

/// Validates that `path` is a regular file or a directory and returns
/// the metadata the decision was based on.
///
/// The metadata is the identity snapshot of the validated file, so a
/// caller that opens the path afterwards can compare the opened handle
/// against it and detect a swap between the two steps.
///
/// # Errors
///
/// Returns the same errors as [`validate_file_type`].
pub fn validated_metadata(path: &Path) -> Result<fs::Metadata, Error> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                escape_path(path)
            )));
        }
    };
    let file_type = metadata.file_type();
    if file_type.is_file() || file_type.is_dir() {
        return Ok(metadata);
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
                escape_path(path)
            )));
        }
    }
    return Ok(());
}

/// Renders an untrusted path as a single printable line.
///
/// A file name cannot be trusted: one containing control characters could
/// otherwise inject terminal escape sequences or forge log lines when the
/// diagnostic is printed, hiding which file actually failed.
///
/// # Examples
///
/// A path containing an escape sequence is rendered with visible escapes
/// instead of emitting the control characters themselves.
pub fn escape_path(path: &Path) -> String {
    let rendered = path.display().to_string();
    return escape_line(&rendered);
}

/// Renders an untrusted name or manifest value as a single printable
/// line, limited to [`MAX_ECHO_CHARS`] characters.
///
/// The value is limited to [`MAX_ECHO_CHARS`] characters before it is
/// escaped, and a truncated value ends with [`ELLIPSIS`]. Control
/// characters cannot therefore split the line or inject terminal escape
/// sequences, and a very long value cannot flood the terminal or a CI
/// log with printable garbage.
///
/// Use this for values that are not file system paths, such as raw
/// command line arguments and the manifest `name`, `version`, and
/// `entrypoint` members, which are echoed in diagnostics.
pub fn escape_text(text: &str) -> String {
    let mut limited = String::new();
    let mut truncated = false;
    for (index, character) in text.chars().enumerate() {
        if index >= MAX_ECHO_CHARS {
            truncated = true;
            break;
        }
        limited.push(character);
    }
    let escaped = escape_line(&limited);
    return match truncated {
        true => format!("{escaped}{ELLIPSIS}"),
        false => escaped,
    };
}

/// Renders untrusted manifest text as printable lines.
///
/// Every control character is escaped like [`escape_text`] does, so a
/// crafted manifest cannot emit terminal escape sequences or carriage
/// returns. Line breaks are kept, because a formatted manifest is
/// printed as a block and would otherwise collapse into one unreadable
/// line. The value is not length-limited: the caller prints the whole
/// manifest, which is bounded by [`crate::common::manifest::MAX_MANIFEST_BYTES`].
///
/// # Examples
///
/// An escape sequence inside a manifest string is rendered with a
/// visible escape instead of being written to the terminal raw.
pub fn escape_manifest(text: &str) -> String {
    let mut rendered = String::new();
    let mut parts = text.split('\n');
    let mut first = true;
    loop {
        let next = parts.next();
        let part = match next {
            None => break,
            Some(value) => value,
        };
        if !first {
            rendered.push('\n');
        }
        let escaped = escape_line(part);
        rendered.push_str(&escaped);
        first = false;
    }
    return rendered;
}

/// Escape a value so that it cannot break a diagnostic across lines.
///
/// Only control characters are escaped, one character at a time. A name
/// therefore cannot split the line or carry terminal escape sequences,
/// while printable characters, including path separators, are left
/// unchanged so that paths stay readable on every platform.
fn escape_line(text: &str) -> String {
    let mut rendered = String::new();
    for character in text.chars() {
        if character.is_control() {
            let escaped = character.escape_debug().to_string();
            rendered.push_str(&escaped);
            continue;
        }
        rendered.push(character);
    }
    return rendered;
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
        escape_path(path)
    ));
}
