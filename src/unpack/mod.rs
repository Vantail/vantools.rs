//! Implementation of the `unpack` command.
//!
//! A run validates the archive with the same inspection the `verify`
//! command runs, then extracts the payload into a temporary directory and
//! renames that directory onto `<target>/<name>-<version>`. Every plugin
//! therefore lands in its own directory, so several plugins can share one
//! parent folder. Validation runs first and writes nothing, so an invalid
//! archive fails before the target is touched, and an existing
//! `<name>-<version>` directory is refused instead of overwritten.

use std::fs;
use std::path::{Path, PathBuf};

use crate::common::Error;
use crate::common::archive;
use crate::common::cli::CliCommand;
use crate::common::manifest::Manifest;
use crate::common::validation::escape_path;

mod writer;

/// The `unpack` command.
///
/// Validates an existing `.van` archive and extracts it into
/// `<target>/<name>-<version>`, creating the target directory when it is
/// missing. The extracted layout matches the source layout `pack`
/// archives: `<target>/<name>-<version>/manifest.json` and the payload
/// below `<target>/<name>-<version>/dist/`. The files are written into a
/// temporary directory inside the target and moved into place with one
/// rename, so the plugin directory appears complete or not at all.
#[non_exhaustive]
pub struct UnpackCommand {
    /// The `.van` archive to extract; must be a regular file.
    pub source: PathBuf,
    /// The directory that receives the `<name>-<version>` directory.
    pub target: PathBuf,
}

/// Everything an unpack run produced.
#[non_exhaustive]
pub struct UnpackReport {
    /// Path of the archive that was extracted.
    pub archive: PathBuf,
    /// Path of the `<name>-<version>` directory the files were written
    /// to.
    pub target: PathBuf,
    /// The parsed and validated manifest.
    pub manifest: Manifest,
    /// Number of files written, including the manifest.
    pub files_written: usize,
}

impl UnpackCommand {
    /// Creates an `unpack` command for the given archive and target.
    pub fn new(source: PathBuf, target: PathBuf) -> Self {
        return Self {
            source,
            target,
        };
    }

    /// Run the command and return what it produced.
    ///
    /// The archive is validated before anything is written, so an
    /// invalid archive leaves the file system untouched. The inspection
    /// and the extraction read one open handle, so a path replaced after
    /// validation cannot change what is extracted. The target directory
    /// is created when it is missing, an existing `<name>-<version>`
    /// directory is refused, and a failed extraction removes its
    /// temporary directory.
    ///
    /// # Errors
    ///
    /// Returns the first validation or runtime error the shared
    /// inspection produces, and the errors of the target creation and
    /// the extraction.
    pub fn run(&self) -> Result<UnpackReport, Error> {
        let mut opened = archive::open(&self.source)?;
        let contents = archive::inspect_archive(&mut opened, &self.source)?;
        let destination = self.destination_path(&contents.manifest);
        ensure_directory(&self.target)?;
        let files_written = writer::extract_archive(
            &mut opened,
            &self.source,
            &destination,
            &contents.manifest_bytes,
        )?;
        return Ok(UnpackReport {
            archive: self.source.clone(),
            target: destination,
            manifest: contents.manifest,
            files_written,
        });
    }

    /// Build the `<target>/<name>-<version>` directory of the plugin.
    fn destination_path(&self, manifest: &Manifest) -> PathBuf {
        let folder_name = format!("{}-{}", manifest.name, manifest.version);
        return self.target.join(folder_name);
    }
}

/// Create `path` together with any missing parent directory.
fn ensure_directory(path: &Path) -> Result<(), Error> {
    let created = fs::create_dir_all(path);
    return match created {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to create target directory {}: {error}",
            escape_path(path)
        ))),
    };
}

impl CliCommand for UnpackCommand {
    fn name(&self) -> &'static str {
        return "unpack";
    }

    fn usage(&self) -> &'static str {
        return "unpack -s <source> -t <target>";
    }

    fn help(&self) -> &'static str {
        return "Usage: unpack -s <source> -t <target>\n\nOptions:\n  -s, --source <path>  The .van archive to extract\n  -t, --target <path>  The target directory to extract into\n  -h, --help           Print this help message\n  -v, --version        Print version information";
    }
}
