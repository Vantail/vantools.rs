//! Manifest handling for `.van` plugin archives.

use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::common::Error;

/// The parsed `manifest.json` from a plugin directory.
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
    /// Reads and parses the `manifest.json` from the given source directory.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when the manifest cannot be read or does
    /// not contain a valid manifest object.
    pub fn from_dir(source: &Path) -> Result<Self, Error> {
        let manifest_path = source.join("manifest.json");
        let content = match fs::read_to_string(&manifest_path) {
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read {}: {error}",
                    manifest_path.display()
                )));
            }
        };
        let parsed: Result<Manifest, serde_json::Error> =
            serde_json::from_str(&content);
        let manifest = match parsed {
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to parse {}: {error}",
                    manifest_path.display()
                )));
            }
        };
        return Ok(manifest);
    }
}
