//! Argument parsing and dispatch for the `unpack` command.

use std::path::PathBuf;

use vantools::common::Error;
use vantools::common::cli::CliCommand;
use vantools::common::validation::{escape_path, escape_text};
use vantools::unpack::{UnpackCommand, UnpackReport};

use super::output::{log_info, log_ok};
use super::parser;
use super::parser::CliAction;

/// Name of the subcommand, used in its usage errors.
const COMMAND_NAME: &str = "unpack";

/// Parse the arguments following `unpack` and run the command.
///
/// Help and version flags are answered with exit code 0. Unknown or
/// incomplete arguments produce a [`Error::CliError`] and exit code 1.
///
/// # Errors
///
/// Returns [`Error::CliError`] when an argument is unknown or a required
/// flag value is missing, and any error of the `unpack` command.
pub fn run_cli(args: &[String]) -> Result<(), Error> {
    let parsed = parser::parse_args(args, COMMAND_NAME, true)?;
    return match parsed.action {
        CliAction::Help => {
            let metadata = UnpackCommand::new(PathBuf::new(), PathBuf::new());
            parser::print_help(metadata.help())
        }
        CliAction::Version => parser::print_version(),
        CliAction::Run => run_unpack_command(parsed.source, parsed.target),
    };
}

/// Resolve the required flags and run the `unpack` command.
///
/// The source flag is reported as missing before the target flag, the
/// order `pack` uses.
fn run_unpack_command(
    source: Option<PathBuf>,
    target: Option<PathBuf>,
) -> Result<(), Error> {
    let resolved_source = match source {
        Some(value) => value,
        None => return Err(parser::missing_arguments(COMMAND_NAME)),
    };
    let resolved_target = match target {
        Some(value) => value,
        None => return Err(parser::missing_arguments(COMMAND_NAME)),
    };
    let command = UnpackCommand::new(resolved_source, resolved_target);
    let report = command.run()?;
    print_report(&report);
    return Ok(());
}

/// Print the report of a completed unpack.
///
/// The check lines keep the wording of the `verify` report, because
/// both commands run the same inspection, and the extraction lines name
/// the archive that was read and the `<name>-<version>` directory that
/// was installed, so the user sees exactly where the plugin landed. Every
/// interpolated path is escaped like any other diagnostic.
fn print_report(report: &UnpackReport) {
    log_ok("Check passed", "Paths and ZIP structure");
    log_ok("Check passed", "manifest.json is present and valid");
    let entrypoint = escape_text(&report.manifest.entrypoint);
    let entrypoint_check =
        format!("dist/ and entrypoint ({entrypoint}) are present");
    log_ok("Check passed", &entrypoint_check);
    log_info("Extracting archive", &escape_path(&report.archive));
    log_ok("Successfully extracted archive", &escape_path(&report.target));
    log_info("Files written", &report.files_written.to_string());
    return;
}
