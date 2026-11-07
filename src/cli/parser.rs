//! Shared argument parsing for the `pack` and `verify` subcommands.
//!
//! The two wrappers differ only in which flags they accept and which
//! values they require. Answering help and version requests, consuming a
//! flag value, and building the usage errors are the same steps in both,
//! so they live here instead of in each wrapper.

use std::io;
use std::path::PathBuf;

use vantools::common::Error;
use vantools::common::cli::version_line;
use vantools::common::validation::escape_text;

use super::output::write_line;

/// Short flag that requests help from a subcommand.
const HELP_FLAG: &str = "-h";

/// Long flag that requests help from a subcommand.
const HELP_LONG_FLAG: &str = "--help";

/// Short flag that requests the version of a subcommand.
const VERSION_FLAG: &str = "-v";

/// Long flag that requests the version of a subcommand.
const VERSION_LONG_FLAG: &str = "--version";

/// Returns true when `argument` requests the help text.
pub fn is_help_flag(argument: &str) -> bool {
    return argument == HELP_FLAG || argument == HELP_LONG_FLAG;
}

/// Returns true when `argument` requests the version banner.
pub fn is_version_flag(argument: &str) -> bool {
    return argument == VERSION_FLAG || argument == VERSION_LONG_FLAG;
}

/// Consume the value that follows a flag.
///
/// `command` names the subcommand the usage error refers to.
///
/// # Errors
///
/// Returns [`Error::CliError`] when the flag is the last argument and
/// no value follows it.
pub fn parse_flag_value<'a, I>(
    remaining: &mut I,
    command: &str,
) -> Result<PathBuf, Error>
where
    I: Iterator<Item = &'a String>,
{
    let value = match remaining.next() {
        Some(value) => value,
        None => return Err(missing_arguments(command)),
    };
    return Ok(PathBuf::from(value));
}

/// Build the error for an invocation without the required flags.
pub fn missing_arguments(command: &str) -> Error {
    return Error::CliError(format!(
        "Missing required arguments. Run 'van {command} --help' for usage."
    ));
}

/// Build the error for an argument a subcommand does not accept.
///
/// The argument is rendered with [`escape_text`], so a control character
/// cannot split the line and a very long value cannot flood the terminal.
pub fn unknown_argument(command: &str, argument: &str) -> Error {
    let rendered = escape_text(argument);
    return Error::CliError(format!(
        "Unknown argument: {rendered}. Run 'van {command} --help' for usage."
    ));
}

/// Print one line to stdout.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when stdout cannot be written.
pub fn print_line(text: &str) -> Result<(), Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    return write_line(&mut handle, text, "output");
}

/// Print the version banner (`van <crate-version>`) to stdout.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when stdout cannot be written.
pub fn print_version() -> Result<(), Error> {
    let line: String = version_line();
    return print_line(&line);
}
