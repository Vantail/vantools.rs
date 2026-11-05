//! Argument parsing and dispatch for the `verify` command.

use std::io;
use std::path::PathBuf;

use crate::common::Error;
use crate::common::cli::{CliCommand, version_line};
use crate::common::command::Command;
use crate::common::output::write_line;
use crate::common::validation::escape_text;

use super::VerifyCommand;

/// Source flag of the `verify` subcommand.
const SOURCE_FLAG: &str = "-s";

/// Long form of the source flag of the `verify` subcommand.
const SOURCE_LONG_FLAG: &str = "--source";

/// Short flag that requests help for the `verify` subcommand.
const HELP_FLAG: &str = "-h";

/// Long flag that requests help for the `verify` subcommand.
const HELP_LONG_FLAG: &str = "--help";

/// Short flag that requests the version of the `verify` subcommand.
const VERSION_FLAG: &str = "-v";

/// Long flag that requests the version of the `verify` subcommand.
const VERSION_LONG_FLAG: &str = "--version";

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
        if argument == HELP_FLAG || argument == HELP_LONG_FLAG {
            return Ok(VerifyRequest::Help);
        }
        if argument == VERSION_FLAG || argument == VERSION_LONG_FLAG {
            return Ok(VerifyRequest::Version);
        }
        let is_source = argument == SOURCE_FLAG || argument == SOURCE_LONG_FLAG;
        if is_source {
            source = Some(parse_flag_value(&mut remaining)?);
            continue;
        }
        return Err(unknown_argument(argument));
    }
    return Ok(VerifyRequest::Run {
        source,
    });
}

/// Consume the value that follows a `-s` flag.
///
/// # Errors
///
/// Returns [`Error::CliError`] when the flag is the last argument and
/// no value follows it.
fn parse_flag_value<'a, I>(remaining: &mut I) -> Result<PathBuf, Error>
where
    I: Iterator<Item = &'a String>,
{
    let value = match remaining.next() {
        Some(value) => value,
        None => return Err(missing_arguments()),
    };
    return Ok(PathBuf::from(value));
}

/// Execute a parsed `verify` request.
fn execute_verify_request(request: VerifyRequest) -> Result<(), Error> {
    return match request {
        VerifyRequest::Help => print_verify_help(),
        VerifyRequest::Version => print_version(),
        VerifyRequest::Run {
            source,
        } => run_verify_command(source),
    };
}

/// Print the full help text of the `verify` command to stdout.
fn print_verify_help() -> Result<(), Error> {
    let metadata = VerifyCommand::new(PathBuf::new());
    return print_line(metadata.help());
}

/// Resolve the required flag and run the `verify` command.
fn run_verify_command(source: Option<PathBuf>) -> Result<(), Error> {
    let resolved_source = match source {
        Some(value) => value,
        None => return Err(missing_arguments()),
    };
    let command = VerifyCommand::new(resolved_source);
    return command.run();
}

/// Build the error for an argument the `verify` command does not accept.
fn unknown_argument(argument: &str) -> Error {
    let rendered = escape_text(argument);
    return Error::CliError(format!(
        "Unknown argument: {rendered}. Run 'van verify --help' for usage."
    ));
}

/// Build the error for a `verify` invocation without a source.
fn missing_arguments() -> Error {
    return Error::CliError(String::from(
        "Missing required arguments. Run 'van verify --help' for usage.",
    ));
}

/// Print one line to stdout.
fn print_line(text: &str) -> Result<(), Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    return write_line(&mut handle, text, "output");
}

/// Print the version banner (`van <crate-version>`) to stdout.
fn print_version() -> Result<(), Error> {
    let line: String = version_line();
    return print_line(&line);
}
