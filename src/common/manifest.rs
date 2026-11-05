//! Manifest handling for `.van` plugin archives.

use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str;

use crate::common::Error;
use crate::common::validation::{
    escape_path, escape_text, validate_archive_entry,
};

/// Byte order mark that a manifest must not start with.
const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Largest `manifest.json` this crate reads, in bytes.
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

/// Largest manifest name accepted, in Unicode scalar values.
///
/// The name becomes the first part of the `<name>-<version>.van`
/// archive file name, and no file system accepts an unbounded file
/// name. Capping it during validation makes the rejection an early,
/// explicit error instead of a late "file name too long" failure after
/// the `dist` collection work is done. 64 matches the echo cap that
/// `van` already applies to untrusted values.
const MAX_NAME_CHARS: usize = 64;

/// Largest manifest version accepted, in Unicode scalar values.
///
/// The version becomes the second part of the archive file name and a
/// SemVer version may carry unbounded build metadata, so like
/// [`MAX_NAME_CHARS`] it bounds the final file name during validation.
const MAX_VERSION_CHARS: usize = 64;

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
    /// entrypoint path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when the file cannot be read, and
    /// [`Error::ValidationError`] when the file is larger than 1 MiB or
    /// its content is invalid.
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
    /// [`Error::ValidationError`] when the file is larger than 1 MiB or
    /// its content is invalid.
    pub fn from_dir_with_bytes(
        source: &Path,
    ) -> Result<(Self, Vec<u8>), Error> {
        let manifest_path = source.join("manifest.json");
        let bytes = read_manifest(&manifest_path)?;
        reject_bom(&bytes)?;
        let content = decode_utf8(&bytes)?;
        let manifest = parse_manifest(content)?;
        validate_manifest(&manifest)?;
        return Ok((manifest, bytes));
    }
}

/// Read the raw manifest bytes.
///
/// At most 1 MiB is read, so an oversized manifest is rejected instead of
/// being loaded into memory in full.
fn read_manifest(path: &Path) -> Result<Vec<u8>, Error> {
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
    let entrypoint = validate_archive_entry(&manifest.entrypoint);
    return match entrypoint {
        Ok(()) => Ok(()),
        Err(_) => Err(Error::ValidationError(format!(
            "Invalid entrypoint path: {}",
            escape_text(&manifest.entrypoint)
        ))),
    };
}

/// Validate the plugin name used in the archive file name.
///
/// The name is joined onto the target directory to build
/// `<name>-<version>.van`, so it must be a single path component: free
/// of path separators, not absolute, without a drive letter, and
/// neither a `.` nor a `..` segment. It must also fit in
/// [`MAX_NAME_CHARS`] Unicode scalar values so the composed file name
/// stays usable.
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
/// it is capped during validation. The version is not rendered in the
/// diagnostic, because an oversized value would flood the terminal,
/// which matches the echo-cap policy elsewhere in the crate.
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

/// Count the Unicode scalar values of `value`.
fn character_count(value: &str) -> usize {
    let mut count: usize = 0;
    for _ in value.chars() {
        count += 1;
    }
    return count;
}
