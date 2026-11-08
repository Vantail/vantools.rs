//! Argument parsing and dispatch for the `verify` command.

use std::path::PathBuf;

use vantools::common::Error;
use vantools::common::cli::CliCommand;
use vantools::tui::progress::show_progress;
use vantools::verify::VerifyCommand;

use super::parser;
use super::parser::CliAction;
use super::verify_report::print_report;

/// Name of the subcommand, used in its usage errors.
const COMMAND_NAME: &str = "verify";

/// Parse the arguments following `verify` and run the command.
///
/// Help and version flags are answered with exit code 0. Unknown or
/// incomplete arguments produce a [`Error::CliError`] and exit code 1.
///
/// # Errors
///
/// Returns [`Error::CliError`] when an argument is unknown or the
/// required flag value is missing, and any error of the `verify`
/// command.
pub fn run_cli(args: &[String]) -> Result<(), Error> {
    let parsed = parser::parse_args(args, COMMAND_NAME, false)?;
    return match parsed.action {
        CliAction::Help => {
            let metadata = VerifyCommand::new(PathBuf::new());
            parser::print_help(metadata.help())
        }
        CliAction::Version => parser::print_version(),
        CliAction::Run => run_verify_command(parsed.source),
    };
}

/// Run `verify` with the source collected by the interactive form.
///
/// The form only submits when the source is non-empty, so the value
/// goes straight to the same runner the parsed flag uses.
///
/// # Errors
///
/// Returns any error of the `verify` command.
pub(crate) fn run_with_path(source: PathBuf) -> Result<(), Error> {
    return run_verify_command(Some(source));
}

/// Resolve the required flag and run the `verify` command.
///
/// The progress bar runs after the flag was accepted, so the help and
/// version output stays untouched.
fn run_verify_command(source: Option<PathBuf>) -> Result<(), Error> {
    let resolved_source = match source {
        Some(value) => value,
        None => return Err(parser::missing_arguments(COMMAND_NAME)),
    };
    let command = VerifyCommand::new(resolved_source);
    show_progress();
    let mut report = command.run()?;
    return print_report(&mut report);
}
