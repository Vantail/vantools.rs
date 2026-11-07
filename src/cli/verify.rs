//! Argument parsing and dispatch for the `verify` command.

use std::path::PathBuf;

use vantools::common::Error;
use vantools::common::cli::CliCommand;
use vantools::verify::VerifyCommand;

use super::parser;
use super::tui::logo::print_logo;
use super::tui::progress::show_progress;
use super::verify_report::print_report;

/// Name of the subcommand, used in its usage errors.
const COMMAND_NAME: &str = "verify";

/// Source flag of the `verify` subcommand.
const SOURCE_FLAG: &str = "-s";

/// Long form of the source flag of the `verify` subcommand.
const SOURCE_LONG_FLAG: &str = "--source";

/// A `verify` invocation resolved into an action to execute.
///
/// The argument scan stops at the first `--help` or `--version` flag
/// without examining later arguments, so the resolution of a scan that
/// stopped early must be distinguishable from a fully parsed run.
enum VerifyRequest {
    /// Run the command with the parsed flag values.
    Run {
        /// Value of the source flag, when given.
        source: Option<PathBuf>,
    },
    /// Print the help text and exit successfully.
    Help,
    /// Print the version banner and exit successfully.
    Version,
}

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
    let request = parse_verify_request(args)?;
    return execute_verify_request(request);
}

/// Scan the arguments and parse the flags into a request.
///
/// The scan stops at the first `--help` or `--version` flag, so later
/// arguments are not examined and a stopped-early request must stay
/// distinguishable from a fully parsed run. A repeated `-s` replaces
/// the earlier value, as before.
fn parse_verify_request(arguments: &[String]) -> Result<VerifyRequest, Error> {
    let mut source: Option<PathBuf> = None;
    let mut remaining = arguments.iter();
    loop {
        let next = remaining.next();
        let argument = match next {
            None => break,
            Some(value) => value.as_str(),
        };
        if parser::is_help_flag(argument) {
            return Ok(VerifyRequest::Help);
        }
        if parser::is_version_flag(argument) {
            return Ok(VerifyRequest::Version);
        }
        let is_source = argument == SOURCE_FLAG || argument == SOURCE_LONG_FLAG;
        if is_source {
            let value = parser::parse_flag_value(&mut remaining, COMMAND_NAME)?;
            source = Some(value);
            continue;
        }
        return Err(parser::unknown_argument(COMMAND_NAME, argument));
    }
    return Ok(VerifyRequest::Run {
        source,
    });
}

/// Execute a parsed `verify` request.
fn execute_verify_request(request: VerifyRequest) -> Result<(), Error> {
    return match request {
        VerifyRequest::Help => print_verify_help(),
        VerifyRequest::Version => parser::print_version(),
        VerifyRequest::Run {
            source,
        } => run_verify_command(source),
    };
}

/// Print the full help text of the `verify` command to stdout.
fn print_verify_help() -> Result<(), Error> {
    let metadata = VerifyCommand::new(PathBuf::new());
    return parser::print_line(metadata.help());
}

/// Resolve the required flag and run the `verify` command.
///
/// The banner and the progress bar run after the flag was accepted, so
/// the help and version output stays untouched.
fn run_verify_command(source: Option<PathBuf>) -> Result<(), Error> {
    let resolved_source = match source {
        Some(value) => value,
        None => return Err(parser::missing_arguments(COMMAND_NAME)),
    };
    let command = VerifyCommand::new(resolved_source);
    print_logo();
    show_progress();
    let mut report = command.run()?;
    return print_report(&mut report);
}
