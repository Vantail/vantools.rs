//! Argument parsing and dispatch for the `unpack` command.

use std::path::PathBuf;

use vantools::common::Error;
use vantools::common::cli::CliCommand;
use vantools::common::validation::escape_path;
use vantools::unpack::{UnpackCommand, UnpackReport};

use super::parser;
use super::tui::log::{log_info, log_ok};
use super::tui::logo::print_logo;
use super::tui::progress::show_progress;

/// Name of the subcommand, used in its usage errors.
const COMMAND_NAME: &str = "unpack";

/// Source flag of the `unpack` subcommand.
const SOURCE_FLAG: &str = "-s";

/// Target flag of the `unpack` subcommand.
const TARGET_FLAG: &str = "-t";

/// Long form of the source flag of the `unpack` subcommand.
const SOURCE_LONG_FLAG: &str = "--source";

/// Long form of the target flag of the `unpack` subcommand.
const TARGET_LONG_FLAG: &str = "--target";

/// Message the scaffold reports in place of an extraction result.
const NOT_IMPLEMENTED_MESSAGE: &str = "Not implemented yet";

/// An `unpack` invocation resolved into an action to execute.
///
/// The argument scan stops at the first `--help` or `--version` flag
/// without examining later arguments, so the resolution of a scan that
/// stopped early must be distinguishable from a fully parsed run.
enum UnpackRequest {
    /// Run the command with the parsed flag values.
    Run {
        /// Value of the source flag, when given.
        source: Option<PathBuf>,
        /// Value of the target flag, when given.
        target: Option<PathBuf>,
    },
    /// Print the help text and exit successfully.
    Help,
    /// Print the version banner and exit successfully.
    Version,
}

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
    let request = parse_unpack_request(args)?;
    return execute_unpack_request(request);
}

/// Scan the arguments and parse the flags into a request.
///
/// The scan stops at the first `--help` or `--version` flag, so later
/// arguments are not examined and a stopped-early request must stay
/// distinguishable from a fully parsed run. A repeated `-s`/`-t`
/// replaces the earlier value.
fn parse_unpack_request(arguments: &[String]) -> Result<UnpackRequest, Error> {
    let mut source: Option<PathBuf> = None;
    let mut target: Option<PathBuf> = None;
    let mut remaining = arguments.iter();
    loop {
        let next = remaining.next();
        let argument = match next {
            None => break,
            Some(value) => value.as_str(),
        };
        if parser::is_help_flag(argument) {
            return Ok(UnpackRequest::Help);
        }
        if parser::is_version_flag(argument) {
            return Ok(UnpackRequest::Version);
        }
        let is_source = argument == SOURCE_FLAG || argument == SOURCE_LONG_FLAG;
        if is_source {
            let value = parser::parse_flag_value(&mut remaining, COMMAND_NAME)?;
            source = Some(value);
            continue;
        }
        let is_target = argument == TARGET_FLAG || argument == TARGET_LONG_FLAG;
        if is_target {
            let value = parser::parse_flag_value(&mut remaining, COMMAND_NAME)?;
            target = Some(value);
            continue;
        }
        return Err(parser::unknown_argument(COMMAND_NAME, argument));
    }
    return Ok(UnpackRequest::Run {
        source,
        target,
    });
}

/// Execute a parsed `unpack` request.
fn execute_unpack_request(request: UnpackRequest) -> Result<(), Error> {
    return match request {
        UnpackRequest::Help => print_unpack_help(),
        UnpackRequest::Version => parser::print_version(),
        UnpackRequest::Run {
            source,
            target,
        } => run_unpack_command(source, target),
    };
}

/// Print the full help text of the `unpack` command to stdout.
fn print_unpack_help() -> Result<(), Error> {
    let metadata = UnpackCommand::new(PathBuf::new(), PathBuf::new());
    return parser::print_line(metadata.help());
}

/// Resolve the required flags and run the `unpack` command.
///
/// The source flag is reported as missing before the target flag, the
/// order `pack` uses. The banner and the progress bar run after the
/// flags were accepted, so the help and version output stays untouched.
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
    print_logo();
    show_progress();
    let report = command.run()?;
    print_report(&report);
    return Ok(());
}

/// Print the report of a scaffolded unpack.
///
/// Nothing is extracted yet, so the success line states that plainly
/// and the requested paths follow as informational lines. Every
/// interpolated path is escaped like any other diagnostic.
fn print_report(report: &UnpackReport) {
    log_ok("Unpacked", NOT_IMPLEMENTED_MESSAGE);
    log_info("Archive", &escape_path(&report.archive));
    log_info("Target directory", &escape_path(&report.target));
    return;
}
