//! Implementation of the `unpack` command.
//!
//! The module is scaffolded only: extraction is not implemented yet.
//! [`UnpackCommand::run`] performs no I/O and returns an
//! [`UnpackReport`] naming the archive it would read and the directory
//! it would fill, so the command, its report, and its CLI wrapper exist
//! before the extraction logic arrives.

use std::path::PathBuf;

use crate::common::Error;
use crate::common::cli::CliCommand;

/// The `unpack` command.
///
/// Extracts an existing `.van` archive into a target directory.
/// Extraction is not implemented yet, so a run reads nothing, writes
/// nothing, and returns a [`UnpackReport`] with the requested archive
/// and target paths.
#[non_exhaustive]
pub struct UnpackCommand {
    /// The `.van` archive to extract.
    pub source: PathBuf,
    /// The directory that receives the extracted files.
    pub target: PathBuf,
}

/// Everything an unpack run produced.
///
/// While the command is a scaffold, the report only names the requested
/// archive and target directory.
#[non_exhaustive]
pub struct UnpackReport {
    /// Path of the archive that was requested.
    pub archive: PathBuf,
    /// Path of the directory the files would be written to.
    pub target: PathBuf,
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
    /// Extraction is not implemented yet: the method performs no I/O
    /// and returns the report straight away.
    ///
    /// # Errors
    ///
    /// Currently never fails. The fallible signature matches the other
    /// commands so the extraction logic can be added without changing
    /// the caller.
    pub fn run(&self) -> Result<UnpackReport, Error> {
        return Ok(UnpackReport {
            archive: self.source.clone(),
            target: self.target.clone(),
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
