//! Implementation of the `unpack` command.
//!
//! A run validates the archive with the same inspection the `verify`
//! command runs, then returns an [`UnpackReport`] with the validated
//! manifest. Extraction itself is not implemented yet: validation runs
//! first and writes nothing, so an invalid archive fails before the
//! target directory would be touched.

use std::path::PathBuf;

use crate::common::Error;
use crate::common::archive;
use crate::common::cli::CliCommand;
use crate::common::manifest::Manifest;

/// The `unpack` command.
///
/// Validates an existing `.van` archive before extracting it into a
/// target directory. Only the validation phase is implemented, so a run
/// reads and validates the archive, writes nothing, and returns an
/// [`UnpackReport`] with the validated manifest.
#[non_exhaustive]
pub struct UnpackCommand {
    /// The `.van` archive to extract; must be a regular file.
    pub source: PathBuf,
    /// The directory that receives the extracted files.
    pub target: PathBuf,
}

/// Everything an unpack run produced.
///
/// Validation is the only phase implemented so far, so the report names
/// the requested archive, the target directory, and the validated
/// manifest.
#[non_exhaustive]
pub struct UnpackReport {
    /// Path of the archive that was validated.
    pub archive: PathBuf,
    /// Path of the directory the files would be written to.
    pub target: PathBuf,
    /// The parsed and validated manifest.
    pub manifest: Manifest,
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
    /// invalid archive leaves the file system untouched: the target
    /// directory is neither created nor filled.
    ///
    /// # Errors
    ///
    /// Returns the first validation or runtime error the shared
    /// inspection produces.
    pub fn run(&self) -> Result<UnpackReport, Error> {
        let contents = archive::inspect(&self.source)?;
        return Ok(UnpackReport {
            archive: self.source.clone(),
            target: self.target.clone(),
            manifest: contents.manifest,
        });
    }
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
