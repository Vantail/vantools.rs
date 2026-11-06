//! Implementation of the `verify` command.
//!
//! The command opens an existing `.van` archive and validates it
//! without extracting anything. The checks themselves live in
//! [`validation`], and the report a valid archive produces lives in
//! [`report`]; this module only holds the command and the data a
//! successful check produced.

use std::path::PathBuf;

use crate::common::Error;
use crate::common::cli::CliCommand;
use crate::common::command::Command;
use crate::common::manifest::Manifest;
use crate::tui::progress::show_progress;

mod cli;
mod report;
mod validation;

pub use cli::run_cli;

/// The `verify` command.
///
/// Opens the archive at `source` and validates its structure without
/// extracting it: every entry name must be a safe relative path and
/// unique within the archive, no entry may carry a `node_modules`
/// component, every entry must be a
/// regular file, every entry must decompress to bytes that match its
/// CRC-32 within the decompressed-size budgets, the archive must hold
/// no more than the entry-count budget, the manifest must pass the
/// same checks `pack` applies, and `dist/` must contain the manifest
/// entrypoint. The archive path itself must be a regular file, so a
/// symbolic link, a FIFO, or a device is refused instead of being
/// opened. A valid archive is reported with its manifest and its
/// contents as a tree.
#[non_exhaustive]
pub struct VerifyCommand {
    /// The `.van` archive to verify; must be a regular file.
    pub source: PathBuf,
}

/// Everything a successful archive check produced.
struct ArchiveCheck {
    /// Entry names as the archive listed them; the report sorts them in
    /// place.
    names: Vec<String>,
    /// The parsed and validated manifest.
    manifest: Manifest,
    /// The exact manifest bytes that passed validation.
    manifest_bytes: Vec<u8>,
}

impl Command for VerifyCommand {
    fn run(&self) -> Result<(), Error> {
        show_progress();
        let mut check = validation::inspect(&self.source)?;
        return report::print_report(&self.source, &mut check);
    }
}

impl VerifyCommand {
    /// Creates a `verify` command for the given archive.
    pub fn new(source: PathBuf) -> Self {
        return Self {
            source,
        };
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
