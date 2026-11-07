//! Implementation of the `verify` command.
//!
//! The command opens an existing `.van` archive and validates it
//! without extracting anything. The checks themselves live in
//! [`validation`]; this module holds the command and the report a
//! successful check produced.

use std::path::PathBuf;

use crate::common::Error;
use crate::common::cli::CliCommand;
use crate::common::manifest::Manifest;

mod validation;

/// The `verify` command.
///
/// Opens the archive at `source` and validates its structure without
/// extracting it: every entry name must be a safe relative path and
/// unique within the archive, no entry may carry a `node_modules`
/// component, every entry must be a regular file or an explicit
/// directory entry, every entry must decompress to bytes that match its
/// CRC-32 within the decompressed-size budgets, the archive must hold
/// no more than the entry-count budget, the manifest must pass the
/// same checks `pack` applies, and `dist/` must contain the manifest
/// entrypoint. The archive path itself must be a regular file, so a
/// symbolic link, a FIFO, or a device is refused instead of being
/// opened. A valid archive produces a [`VerifyReport`] with its
/// manifest and its entry names.
#[non_exhaustive]
pub struct VerifyCommand {
    /// The `.van` archive to verify; must be a regular file.
    pub source: PathBuf,
}

/// Everything a successful archive check produced.
#[non_exhaustive]
pub struct VerifyReport {
    /// The archive that was checked.
    pub source: PathBuf,
    /// The parsed and validated manifest.
    pub manifest: Manifest,
    /// The exact manifest bytes that passed validation.
    pub manifest_bytes: Vec<u8>,
    /// Entry names as the archive listed them, in archive order.
    pub entry_names: Vec<String>,
}

impl VerifyCommand {
    /// Creates a `verify` command for the given archive.
    pub fn new(source: PathBuf) -> Self {
        return Self {
            source,
        };
    }

    /// Run the command and return what it found.
    ///
    /// The command writes nothing; the caller decides how to report the
    /// result.
    ///
    /// # Errors
    ///
    /// Returns the first validation or runtime error the checks produce.
    pub fn run(&self) -> Result<VerifyReport, Error> {
        return validation::inspect(&self.source);
    }
}

impl CliCommand for VerifyCommand {
    fn name(&self) -> &'static str {
        return "verify";
    }

    fn usage(&self) -> &'static str {
        return "verify -s <source>";
    }

    fn help(&self) -> &'static str {
        return "Usage: verify -s <source>\n\nOptions:\n  -s, --source <path>  The .van archive to verify (a regular file)\n  -h, --help           Print this help message\n  -v, --version        Print version information";
    }
}
