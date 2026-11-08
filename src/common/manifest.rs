//! Manifest handling for `.van` plugin archives.

use serde::Deserialize;
use std::io::Read;
use std::path::Path;
use std::str;

use crate::common::Error;
use crate::common::validation::{
    escape_path, escape_text, open_validated_file, validate_archive_entry,
};

/// Byte order mark that a manifest must not start with.
const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Largest `manifest.json` this crate reads, in bytes.
pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

/// Largest manifest name accepted, in Unicode scalar values.
///
/// The name becomes the first part of the `<name>-<version>.van`
/// archive file name, and no file system accepts an unbounded file
/// name. Capping it during validation makes the rejection an early,
/// explicit error instead of a late "file name too long" failure after
/// the `dist` collection work is done. 64 matches the echo cap that
/// `van` already applies to untrusted values. A character count does
/// not bound bytes, so the composed file name is also measured in
/// bytes against [`MAX_ARCHIVE_NAME_BYTES`].
const MAX_NAME_CHARS: usize = 64;

/// Largest manifest version accepted, in Unicode scalar values.
///
/// The version becomes the second part of the archive file name and a
/// SemVer version may carry unbounded build metadata, so like
/// [`MAX_NAME_CHARS`] it bounds the final file name during validation.
/// The composed file name is bounded in bytes separately, by
/// [`MAX_ARCHIVE_NAME_BYTES`].
const MAX_VERSION_CHARS: usize = 64;

/// Largest archive file name accepted, in bytes.
///
/// `<name>-<version>.van` becomes a single file name component, and
/// common file systems limit one component to 255 bytes. The
/// per-field caps count characters, not bytes, and one character can
/// need four bytes, so name and version can both pass their own caps
/// and still compose a file name the file system refuses. Measuring
/// the composed name in bytes during validation keeps the rejection
/// an early, explicit [`Error::ValidationError`] instead of a late
/// runtime failure after the `dist` collection work is done.
const MAX_ARCHIVE_NAME_BYTES: usize = 255;

/// Bytes the archive file name adds around the two manifest fields.
///
/// `<name>-<version>.van` adds a hyphen and the `.van` extension on
/// top of the name and the version.
const ARCHIVE_NAME_OVERHEAD: usize = "-.van".len();

/// The parsed and validated `manifest.json` from a plugin directory.
#[derive(Deserialize)]
#[non_exhaustive]
pub struct Manifest {
    /// The plugin name, used as the first part of the archive name.
    pub name: String,
    /// The plugin version, used as the second part of the archive name.
    pub version: String,
    /// The plugin entry file, relative to the `dist/` directory.
    pub entrypoint: String,
}

impl Manifest {
    /// Reads, parses, and validates the `manifest.json` of a source
    /// directory.
    ///
    /// The manifest must be UTF-8 without a byte order mark, contain a
    /// non-empty bounded-length name and version, and a relative
    /// entrypoint path that names a file.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when the file cannot be read, and
    /// [`Error::ValidationError`] when the path is not a regular file,
    /// the file is larger than 1 MiB, or its content is invalid.
    pub fn from_dir(source: &Path) -> Result<Self, Error> {
        let loaded = Self::from_dir_with_bytes(source)?;
        let (manifest, _) = loaded;
        return Ok(manifest);
    }

    /// Reads and validates the `manifest.json` of a source directory and
    /// returns it together with the exact bytes that were validated.
    ///
    /// Callers that copy the manifest into an archive should write these
    /// bytes instead of reading the file again, so the archived manifest
    /// is always the one that passed validation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when the file cannot be read, and
    /// [`Error::ValidationError`] when the path is not a regular file,
    /// the file is larger than 1 MiB, or its content is invalid.
    pub fn from_dir_with_bytes(
        source: &Path,
    ) -> Result<(Self, Vec<u8>), Error> {
        let manifest_path = source.join("manifest.json");
        let bytes = read_manifest(&manifest_path)?;
        let manifest = Self::from_bytes(&bytes)?;
        return Ok((manifest, bytes));
    }

    /// Validates and parses manifest bytes that were already read.
    ///
    /// The bytes must be UTF-8 without a byte order mark and at most
    /// [`MAX_MANIFEST_BYTES`] long, and the parsed manifest must carry a
    /// usable name and version and a relative entrypoint path. These are
    /// the checks [`Manifest::from_dir_with_bytes`] applies to a
    /// manifest on disk, so a manifest read from an archive is validated
    /// with the same rules.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ValidationError`] when the bytes exceed
    /// [`MAX_MANIFEST_BYTES`] or their content is invalid.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(Error::ValidationError(String::from(
                "Manifest is larger than 1 MiB",
            )));
        }
        reject_bom(bytes)?;
        let content = decode_utf8(bytes)?;
        let manifest = parse_manifest(content)?;
        validate_manifest(&manifest)?;
        return Ok(manifest);
    }
}

/// Read the raw manifest bytes through a validated handle.
///
/// The file is opened by [`open_validated_file`], so the manifest read
/// applies the same identity check as the `dist` files and a path
/// swapped after validation cannot supply a manifest from outside the
/// source directory. At most 1 MiB is read, so an oversized manifest is
/// rejected instead of being loaded into memory in full.
fn read_manifest(path: &Path) -> Result<Vec<u8>, Error> {
    let file = open_validated_file(path)?;
    let mut limited = file.take(MAX_MANIFEST_BYTES + 1);
    let mut bytes: Vec<u8> = Vec::new();
    let read = limited.read_to_end(&mut bytes);
    match read {
        Ok(_) => {}
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to read {}: {error}",
                escape_path(path)
            )));
        }
    }
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(Error::ValidationError(format!(
            "Manifest is larger than 1 MiB: {}",
            escape_path(path)
        )));
    }
    return Ok(bytes);
}

/// Reject a manifest that starts with a UTF-8 byte order mark.
fn reject_bom(bytes: &[u8]) -> Result<(), Error> {
    if bytes.starts_with(BOM.as_slice()) {
        return Err(Error::ValidationError(String::from(
            "Manifest must not contain a byte order mark (BOM)",
        )));
    }
    return Ok(());
}

/// Decode the manifest bytes as UTF-8.
fn decode_utf8(bytes: &[u8]) -> Result<&str, Error> {
    let decoded = str::from_utf8(bytes);
    return match decoded {
        Ok(value) => Ok(value),
        Err(_) => Err(Error::ValidationError(String::from(
            "Manifest is not valid UTF-8",
        ))),
    };
}

/// Parse the decoded manifest text as JSON.
fn parse_manifest(content: &str) -> Result<Manifest, Error> {
    let parsed: Result<Manifest, serde_json::Error> =
        serde_json::from_str(content);
    return match parsed {
        Ok(value) => Ok(value),
        Err(error) => Err(Error::ValidationError(format!(
            "Manifest is not valid JSON: {error}"
        ))),
    };
}

/// Validate the required manifest members.
fn validate_manifest(manifest: &Manifest) -> Result<(), Error> {
    validate_name(&manifest.name)?;
    validate_version_length(&manifest.version)?;
    validate_archive_name_length(&manifest.name, &manifest.version)?;
    let version = semver::Version::parse(&manifest.version);
    match version {
        Ok(_) => {}
        Err(_) => {
            return Err(Error::ValidationError(format!(
                "Invalid SemVer version: {}",
                escape_text(&manifest.version)
            )));
        }
    }
    let entrypoint = validate_entrypoint(&manifest.entrypoint);
    return match entrypoint {
        Ok(()) => Ok(()),
        Err(_) => Err(Error::ValidationError(format!(
            "Invalid entrypoint path: {}",
            escape_text(&manifest.entrypoint)
        ))),
    };
}

/// Validate the manifest entrypoint as a relative archive path.
///
/// The entrypoint names the plugin entry file below `dist/`, so it must
/// name a file rather than a directory. [`validate_archive_entry`]
/// strips one trailing slash because a standard ZIP archive marks a
/// directory entry that way, which would let an `app.ts/` entrypoint be
/// satisfied by a `dist/app.ts/` directory entry that carries no
/// payload. The marker is therefore rejected here explicitly, so the
/// check the `pack` command applies to the file on disk and the check
/// `verify` and `unpack` apply to the archive entry agree.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when the entrypoint ends with a
/// directory marker, and when it violates an archive path rule.
fn validate_entrypoint(entrypoint: &str) -> Result<(), Error> {
    if entrypoint.ends_with('/') {
        return Err(Error::ValidationError(String::from(
            "Entrypoint must not end with a slash",
        )));
    }
    let checked = validate_archive_entry(entrypoint);
    return checked;
}

/// Validate the plugin name used in the archive file name.
///
/// The name is joined onto the target directory to build
/// `<name>-<version>.van`, so it must be a single path component: free
/// of path separators, not absolute, without a drive letter, and
/// neither a `.` nor a `..` segment. It must also fit in
/// [`MAX_NAME_CHARS`] Unicode scalar values, and together with the
/// version it must compose a file name within
/// [`MAX_ARCHIVE_NAME_BYTES`] bytes, so the composed name stays usable.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when the name is empty, too
/// long, or not usable as a file name.
fn validate_name(name: &str) -> Result<(), Error> {
    if name.is_empty() {
        return Err(Error::ValidationError(String::from(
            "Manifest name must not be empty",
        )));
    }
    if character_count(name) > MAX_NAME_CHARS {
        return Err(Error::ValidationError(format!(
            "Manifest name must not exceed {MAX_NAME_CHARS} characters"
        )));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(invalid_name(name));
    }
    let checked = validate_archive_entry(name);
    return match checked {
        Ok(()) => Ok(()),
        Err(_) => Err(invalid_name(name)),
    };
}

/// Build the error for a name that cannot be used as a file name.
fn invalid_name(name: &str) -> Error {
    return Error::ValidationError(format!(
        "Invalid manifest name: {}",
        escape_text(name)
    ));
}

/// Validate that the version string is short enough for the archive
/// file name.
///
/// The version is the second part of `<name>-<version>.van` and a
/// SemVer version may carry unbounded build metadata, so like the name
/// it is capped during validation. The composed file name is bounded
/// in bytes by [`validate_archive_name_length`]. The version is not
/// rendered in the diagnostic, because an oversized value would flood
/// the terminal, which matches the echo-cap policy elsewhere in the
/// crate.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when the version exceeds
/// [`MAX_VERSION_CHARS`] Unicode scalar values.
fn validate_version_length(version: &str) -> Result<(), Error> {
    if character_count(version) > MAX_VERSION_CHARS {
        return Err(Error::ValidationError(format!(
            "Manifest version must not exceed {MAX_VERSION_CHARS} characters"
        )));
    }
    return Ok(());
}

/// Validate that the composed archive file name fits a file system
/// name component.
///
/// The per-field caps count characters, and one character can need up
/// to four bytes, so a name and a version that both pass
/// [`validate_name`] and [`validate_version_length`] can still compose
/// a `<name>-<version>.van` file name above `NAME_MAX`. Checking the
/// composed length here keeps that rejection an early validation
/// error naming the manifest condition, rather than a late file
/// system failure of a different error class.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when the composed name exceeds
/// [`MAX_ARCHIVE_NAME_BYTES`] bytes. The oversized fields are not
/// echoed, matching the echo policy of the other length checks.
fn validate_archive_name_length(
    name: &str,
    version: &str,
) -> Result<(), Error> {
    let composed = name.len() + version.len() + ARCHIVE_NAME_OVERHEAD;
    if composed > MAX_ARCHIVE_NAME_BYTES {
        return Err(Error::ValidationError(format!(
            "Manifest name and version must not exceed {MAX_ARCHIVE_NAME_BYTES} bytes in total"
        )));
    }
    return Ok(());
}

/// Count the Unicode scalar values of `value`.
fn character_count(value: &str) -> usize {
    let mut count: usize = 0;
    for _ in value.chars() {
        count += 1;
    }
    return count;
}
