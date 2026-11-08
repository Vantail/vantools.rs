//! Reusable path validation rules for `.van` plugin archives.
//!
//! The checks implement Sections 3 and 6 of `docs/SPEC.md` so that every
//! command that reads or writes archives can share one implementation.
//!
//! This module also renders untrusted names and paths for diagnostics, so
//! every command reports a bad file name the same safe way.

use std::ffi::OsStr;
use std::fs;
use std::fs::File;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::Path;

#[cfg(windows)]
use same_file::Handle;
use unicode_normalization::is_nfc;

use crate::common::Error;

/// Directory name that archives must not contain at any depth.
const FORBIDDEN_DIRECTORY: &str = "node_modules";

/// Largest number of characters of an untrusted value echoed in a
/// diagnostic.
///
/// Validation and CLI errors interpolate untrusted values into their
/// text: a raw command line argument, a manifest `name` member, or a
/// file system path.
/// Capping the rendered value keeps an oversized but legal value from
/// flooding the terminal or CI log with printable garbage, matching the
/// bound the `van` dispatcher already applies. Truncated values are
/// marked with [`ELLIPSIS`].
pub const MAX_ECHO_CHARS: usize = 64;

/// Marker appended to a rendered untrusted value that was truncated.
const ELLIPSIS: &str = "...";

/// File names Windows reserves for devices.
///
/// A name in this list resolves to a device rather than to a regular
/// file, in every directory and with or without an extension, so an
/// archive entry with such a name cannot be extracted to the file its
/// own name describes.
const RESERVED_DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
    "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
    "LPT7", "LPT8", "LPT9",
];

/// Validates a raw relative archive entry path.
///
/// A single trailing slash is accepted, because a standard ZIP archive
/// lists a directory as an entry whose name ends with `/`. Only that
/// marker is ignored: the rest of the path must still be a relative,
/// NFC path without an empty segment.
///
/// Rejects backslashes, absolute paths, drive letters, empty path
/// segments, `.` and `..` traversal segments, paths that are not
/// normalized to Unicode NFC, Windows reserved device names, segments
/// that end with a dot or a space, and (on Windows builds) colons.
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
    let entry_path = strip_directory_marker(path_str);
    if is_absolute_or_has_empty_segment(entry_path) {
        return Err(Error::ValidationError(format!(
            "Absolute paths or drive letters are not allowed: {}",
            escape_text(path_str)
        )));
    }
    if has_traversal_segment(entry_path) {
        return Err(Error::ValidationError(format!(
            "Path traversal segments are not allowed: {}",
            escape_text(path_str)
        )));
    }
    if !is_nfc(entry_path) {
        return Err(Error::ValidationError(format!(
            "Path is not in Unicode NFC format: {}",
            escape_text(path_str)
        )));
    }
    validate_windows_segments(entry_path)?;
    return Ok(());
}

/// Returns the entry path without the trailing slash that marks a ZIP
/// directory entry.
///
/// At most one slash is removed, so a name such as `dist//` keeps a
/// trailing empty segment and is still rejected as empty.
fn strip_directory_marker(path_str: &str) -> &str {
    let stripped = path_str.strip_suffix('/');
    return match stripped {
        Some(value) => value,
        None => path_str,
    };
}

/// Validates every segment of a relative archive path as a portable
/// Windows file name.
///
/// Win32 resolves a reserved device name to a device instead of a file,
/// removes a trailing dot or space from every component of a path, and
/// reads a colon after the first character of a component as the start
/// of a named data stream. An entry subject to any of these rules
/// therefore does not name the file it was verified as once it is
/// extracted on Windows, which is the hazard the Unicode NFC rule
/// covers for macOS.
///
/// The colon is rejected only on Windows builds, because it is an
/// ordinary file name character elsewhere and a name such as `a:b.js`
/// refers to a real file there.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] for the first segment that is a
/// reserved device name, ends with a dot or a space, or names an
/// alternate data stream.
fn validate_windows_segments(entry_path: &str) -> Result<(), Error> {
    for segment in entry_path.split('/') {
        if is_reserved_device_name(segment) {
            return Err(Error::ValidationError(format!(
                "Reserved device names are not allowed in paths: {}",
                escape_text(entry_path)
            )));
        }
        if ends_with_dot_or_space(segment) {
            return Err(Error::ValidationError(format!(
                "Trailing dots or spaces are not allowed in paths: {}",
                escape_text(entry_path)
            )));
        }
        if names_alternate_data_stream(segment) {
            return Err(Error::ValidationError(format!(
                "Colons are not allowed in paths: {}",
                escape_text(entry_path)
            )));
        }
    }
    return Ok(());
}

/// Returns true when `segment` names a reserved Windows device.
///
/// The comparison ignores ASCII case and stops at the first dot or
/// colon, because Win32 treats `con`, `Con`, `con.txt`, and `CON:x`
/// alike while `console` is an ordinary file name.
fn is_reserved_device_name(segment: &str) -> bool {
    let before_extension = match segment.split_once('.') {
        Some(value) => value.0,
        None => segment,
    };
    let base = match before_extension.split_once(':') {
        Some(value) => value.0,
        None => before_extension,
    };
    for reserved in RESERVED_DEVICE_NAMES {
        if base.eq_ignore_ascii_case(reserved) {
            return true;
        }
    }
    return false;
}

/// Returns true when `segment` ends with a dot or a space.
///
/// Win32 drops both characters when it resolves a path, so such a
/// segment would not name the file the archive lists.
fn ends_with_dot_or_space(segment: &str) -> bool {
    return segment.ends_with('.') || segment.ends_with(' ');
}

/// Returns true when `segment` would name an alternate data stream.
///
/// Win32 reads a colon after the first character of a component as the
/// start of a named data stream, so an entry named `dist/app.ts:payload`
/// writes into a stream of `dist/app.ts` instead of creating the file
/// the archive lists. The stream is hidden from directory listings and
/// persists attached to the file, so the extracted tree would not match
/// the names that were validated.
///
/// Only Windows builds reject the colon: it is an ordinary character
/// in a Unix file name, and a name such as `a:b.js` refers to a real
/// file there. A Windows extraction rejects that archive before any
/// entry is written.
#[cfg(windows)]
fn names_alternate_data_stream(segment: &str) -> bool {
    return segment.contains(':');
}

/// Returns true when `segment` would name an alternate data stream.
///
/// A colon names no stream outside Win32, so other builds accept it and
/// a name such as `a:b.js` keeps referring to a real file.
#[cfg(not(windows))]
fn names_alternate_data_stream(_segment: &str) -> bool {
    return false;
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

/// Opens `path` only when it still refers to the file that passed
/// validation.
///
/// The identity of the entry is captured immediately before the path is
/// opened, and the opened handle is compared with that identity. A path
/// swapped for a symlink or another file between the two steps is
/// therefore rejected instead of contributing content from elsewhere,
/// both around and inside the check-then-open window.
///
/// The comparison is exact on Unix, by device and inode numbers, and on
/// Windows, by volume serial number and file index. On the remaining
/// platforms stable `std` exposes no file identity, so only the file
/// type of the opened handle is checked.
///
/// Every caller that reads a file from a directory it does not fully
/// control should open it this way, so the identity check is applied
/// consistently instead of in one place only.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] for a symbolic link, another
/// non-regular file, or a path that was replaced after validation, and
/// [`Error::Runtime`] when the path cannot be opened or inspected.
pub fn open_validated_file(path: &Path) -> Result<File, Error> {
    let identity = validated_identity(path)?;
    let opened = File::open(path);
    let file = match opened {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to read {}: {error}",
                escape_path(path)
            )));
        }
    };
    if !is_same_validated_file(&identity, &file, path)? {
        return Err(Error::ValidationError(format!(
            "Source file changed or was replaced while packing: {}",
            escape_path(path)
        )));
    }
    return Ok(file);
}

/// Identity of the entry that passed validation.
///
/// The value is captured before the path is opened and compared with the
/// opened handle afterwards, so a path replaced in between is detected.
#[cfg(unix)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

/// Identity of the entry that passed validation.
///
/// Stable `std` exposes no device and inode numbers on Windows, so the
/// identity is a handle to the file the path resolved to. Comparing two
/// handles is exact: the `same-file` crate reads the volume serial
/// number and file index the platform associates with each one.
#[cfg(windows)]
struct FileIdentity {
    handle: Handle,
}

/// Identity placeholder for platforms without an identity check.
///
/// See [`is_same_validated_file`] for why only the file type is
/// compared there.
#[cfg(not(any(unix, windows)))]
struct FileIdentity;

/// Validates `path` and captures the identity of the entry it names.
///
/// On Unix a single `lstat` both rejects symbolic links and non-regular
/// files and supplies the device and inode numbers, so the identity
/// covers the whole window around the open.
///
/// On Windows stable `std` exposes no comparable identity, so a handle
/// is opened first and the validation follows. Validating first would
/// leave a window in which a symbolic link planted before the handle is
/// opened would be followed both by the handle and by the open that
/// reads the content, which is the swap this check exists to catch.
///
/// # Errors
///
/// Returns the same errors as [`validated_metadata`], and
/// [`Error::Runtime`] when the identity cannot be read.
#[cfg(unix)]
fn validated_identity(path: &Path) -> Result<FileIdentity, Error> {
    let validated = validated_metadata(path)?;
    return Ok(FileIdentity {
        device: validated.dev(),
        inode: validated.ino(),
    });
}

/// Validates `path` and captures the identity of the entry it names.
///
/// See the Unix variant of this function for why the handle is opened
/// before the entry is validated.
///
/// # Errors
///
/// Returns the same errors as [`validated_metadata`], and
/// [`Error::Runtime`] when the identity cannot be read.
#[cfg(windows)]
fn validated_identity(path: &Path) -> Result<FileIdentity, Error> {
    let opened = Handle::from_path(path);
    let handle = match opened {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                escape_path(path)
            )));
        }
    };
    // The handle above identifies the target; this call rejects a
    // symbolic link or a non-regular file sitting at the path now.
    validated_metadata(path)?;
    return Ok(FileIdentity {
        handle,
    });
}

/// Validates `path` and captures the identity of the entry it names.
///
/// See the Unix variant of this function for why only the file type can
/// be compared on this platform.
///
/// # Errors
///
/// Returns the same errors as [`validated_metadata`].
#[cfg(not(any(unix, windows)))]
fn validated_identity(path: &Path) -> Result<FileIdentity, Error> {
    validated_metadata(path)?;
    return Ok(FileIdentity);
}

/// Returns true when `file` still refers to the entry that passed
/// validation.
///
/// A validated entry and a swap target are different file system
/// entries. On Unix the opened handle is compared with the device and
/// inode numbers captured during validation. On Windows the two handles
/// are compared by volume serial number and file index. Both are exact,
/// so a swap is rejected instead of being read.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the opened handle cannot be
/// inspected.
#[cfg(unix)]
fn is_same_validated_file(
    identity: &FileIdentity,
    file: &File,
    path: &Path,
) -> Result<bool, Error> {
    let opened = match file.metadata() {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                escape_path(path)
            )));
        }
    };
    let same_device = identity.device == opened.dev();
    let same_inode = identity.inode == opened.ino();
    return Ok(same_device && same_inode);
}

/// Returns true when `file` still refers to the entry that passed
/// validation.
///
/// The identity captured during validation and the opened handle are
/// compared by the `same-file` crate, which reads the volume serial
/// number and file index of each. A duplicate handle is taken because
/// the comparison consumes it while the caller keeps reading the file.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when a duplicate handle cannot be taken
/// or inspected.
#[cfg(windows)]
fn is_same_validated_file(
    identity: &FileIdentity,
    file: &File,
    path: &Path,
) -> Result<bool, Error> {
    let duplicate = match file.try_clone() {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                escape_path(path)
            )));
        }
    };
    let opened = match Handle::from_file(duplicate) {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                escape_path(path)
            )));
        }
    };
    return Ok(identity.handle == opened);
}

/// Returns true when the opened handle refers to a regular file.
///
/// See the Unix variant of this function for why stable `std` offers no
/// exact file identity on this platform.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the opened handle cannot be
/// inspected.
#[cfg(not(any(unix, windows)))]
fn is_same_validated_file(
    _identity: &FileIdentity,
    file: &File,
    path: &Path,
) -> Result<bool, Error> {
    let opened = match file.metadata() {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                escape_path(path)
            )));
        }
    };
    return Ok(opened.file_type().is_file());
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

/// Renders an untrusted path as a single printable line, limited to
/// [`MAX_ECHO_CHARS`] characters.
///
/// A file name cannot be trusted: one containing control characters
/// could otherwise inject terminal escape sequences or forge log lines
/// when the diagnostic is printed, hiding which file actually failed.
/// A path also arrives from an argument or a directory entry and is not
/// length-bounded, so the rendered value is bounded exactly like
/// [`escape_text`] bounds an argument or a manifest value: a truncated
/// path ends with [`ELLIPSIS`] and cannot flood the terminal or a CI
/// log with one arbitrarily long line.
///
/// # Examples
///
/// A path containing an escape sequence is rendered with visible
/// escapes instead of emitting the control characters themselves.
pub fn escape_path(path: &Path) -> String {
    let rendered = path.display().to_string();
    return escape_text(&rendered);
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
/// Use this for every untrusted value echoed in a diagnostic: raw
/// command line arguments, the manifest `name`, `version`, and
/// `entrypoint` members, and file system paths, which arrive through
/// [`escape_path`].
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

/// Returns true when the path is absolute, starts with a Windows drive
/// letter, or has an empty segment.
///
/// A colon is rejected here only as part of a drive letter, so a Unix
/// file name that contains one, such as `dist/a:b.js`, is left to the
/// segment checks, which reject it on Windows builds.
fn is_absolute_or_has_empty_segment(path_str: &str) -> bool {
    if path_str.starts_with('/') {
        return true;
    }
    if has_drive_letter_prefix(path_str) {
        return true;
    }
    for segment in path_str.split('/') {
        if segment.is_empty() {
            return true;
        }
    }
    return false;
}

/// Returns true when the path starts with a drive letter such as `C:`.
///
/// The colon counts as a drive letter only at index 1, after an ASCII
/// alphabetic character. A colon anywhere else in the path is a legal
/// Unix file name character and is left to the segment checks.
fn has_drive_letter_prefix(path_str: &str) -> bool {
    let bytes = path_str.as_bytes();
    let letter = match bytes.first() {
        Some(value) => *value,
        None => return false,
    };
    if !letter.is_ascii_alphabetic() {
        return false;
    }
    let colon = match bytes.get(1) {
        Some(value) => *value,
        None => return false,
    };
    return colon == b':';
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
