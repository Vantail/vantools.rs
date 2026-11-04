//! Manifest handling for `.van` plugin archives.

use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::str;

use crate::common::Error;
use crate::common::validation::validate_archive_entry;

/// Byte order mark that a manifest must not start with.
const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

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
    /// non-empty name, a SemVer 2.0.0 version, and a relative
    /// entrypoint path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when the file cannot be read, and
    /// [`Error::ValidationError`] when its content is invalid.
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
    /// [`Error::ValidationError`] when its content is invalid.
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
fn read_manifest(path: &Path) -> Result<Vec<u8>, Error> {
    let read = fs::read(path);
    return match read {
        Ok(value) => Ok(value),
        Err(error) => Err(Error::Runtime(format!(
            "failed to read {}: {error}",
            path.display()
        ))),
    };
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
    let version = semver::Version::parse(&manifest.version);
    match version {
        Ok(_) => {}
        Err(_) => {
            return Err(Error::ValidationError(format!(
                "Invalid SemVer version: {}",
                manifest.version
            )));
        }
    }
    let entrypoint = validate_archive_entry(&manifest.entrypoint);
    return match entrypoint {
        Ok(()) => Ok(()),
        Err(_) => Err(Error::ValidationError(format!(
            "Invalid entrypoint path: {}",
            manifest.entrypoint
        ))),
    };
}

/// Validate the plugin name used in the archive file name.
///
/// The name is joined onto the target directory to build
/// `<name>-<version>.van`, so it must be a single path component: free
/// of path separators, not absolute, without a drive letter, and
/// neither a `.` nor a `..` segment.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when the name is empty or is not
/// usable as a file name.
fn validate_name(name: &str) -> Result<(), Error> {
    if name.is_empty() {
        return Err(Error::ValidationError(String::from(
            "Manifest name must not be empty",
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
    return Error::ValidationError(format!("Invalid manifest name: {name}"));
}
