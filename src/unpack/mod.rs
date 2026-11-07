//! Implementation of the `unpack` command.
//!
//! A run validates the archive with the same inspection the `verify`
//! command runs, then extracts the payload into a temporary sibling
//! directory and renames that directory onto the target. Validation runs
//! first and writes nothing, so an invalid archive fails before the
//! target directory is touched, and an existing target is refused
//! instead of overwritten.

use std::path::PathBuf;

use crate::common::Error;
use crate::common::archive;
use crate::common::cli::CliCommand;
use crate::common::manifest::Manifest;

mod writer;

/// The `unpack` command.
///
/// Validates an existing `.van` archive and extracts it into a target
/// directory that must not exist yet. The extracted layout matches the
/// source layout `pack` archives: `<target>/manifest.json` and the
/// payload below `<target>/dist/`. The files are written into a
/// temporary sibling directory and moved into place with one rename, so
/// the target appears complete or not at all.
#[non_exhaustive]
pub struct UnpackCommand {
    /// The `.van` archive to extract; must be a regular file.
    pub source: PathBuf,
    /// The directory that receives the extracted files.
    pub target: PathBuf,
}

/// Everything an unpack run produced.
#[non_exhaustive]
pub struct UnpackReport {
    /// Path of the archive that was extracted.
    pub archive: PathBuf,
    /// Path of the directory the files were written to.
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
    /// invalid archive leaves the file system untouched. A target that
    /// already exists is refused, and a failed extraction removes its
    /// temporary directory.
    ///
    /// # Errors
    ///
    /// Returns the first validation or runtime error the shared
    /// inspection produces, and the errors of the extraction.
    pub fn run(&self) -> Result<UnpackReport, Error> {
        let contents = archive::inspect(&self.source)?;
        let files_written = writer::extract_archive(
            &self.source,
            &self.target,
            &contents.manifest_bytes,
        )?;
        return Ok(UnpackReport {
            archive: self.source.clone(),
            target: self.target.clone(),
            manifest: contents.manifest,
            files_written,
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
