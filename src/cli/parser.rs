//! Shared argument parsing for the `pack`, `unpack`, and `verify`
//! subcommands.
//!
//! The wrappers differ only in whether they accept a target flag and
//! which values they require. Answering help and version requests,
//! consuming a flag value, and building the usage errors are the same
//! steps in all of them, so they live here instead of in each wrapper.

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

/// Short flag naming the source of a subcommand.
pub const SOURCE_FLAG: &str = "-s";

/// Long flag naming the source of a subcommand.
pub const SOURCE_LONG_FLAG: &str = "--source";

/// Short flag naming the target of a subcommand.
pub const TARGET_FLAG: &str = "-t";

/// Long flag naming the target of a subcommand.
pub const TARGET_LONG_FLAG: &str = "--target";

/// What a subcommand invocation should do.
pub enum CliAction {
    /// Run the command with the parsed flag values.
    Run,
    /// Print the help text and exit successfully.
    Help,
    /// Print the version banner and exit successfully.
    Version,
}

/// Flag values and action parsed from a subcommand invocation.
///
/// The scan stops at the first `--help` or `--version` flag without
/// examining later arguments, so a resolved action stays distinguishable
/// from a fully parsed run.
pub struct ParsedArgs {
    /// Value of the source flag, when given.
    pub source: Option<PathBuf>,
    /// Value of the target flag, when given.
    pub target: Option<PathBuf>,
    /// What the subcommand should do.
    pub action: CliAction,
}

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

/// Parse the arguments of a subcommand into a [`ParsedArgs`].
///
/// The scan stops at the first `--help` or `--version` flag, so later
/// arguments are not examined. A repeated `-s`/`-t` replaces the earlier
/// value. When `allow_target` is false the target flags are rejected as
/// unknown arguments, which is what `verify` requires.
///
/// # Errors
///
/// Returns [`Error::CliError`] when an argument is unknown or a flag
/// value is missing.
pub fn parse_args(
    arguments: &[String],
    command_name: &str,
    allow_target: bool,
) -> Result<ParsedArgs, Error> {
    let mut source: Option<PathBuf> = None;
    let mut target: Option<PathBuf> = None;
    let mut remaining = arguments.iter();
    loop {
        let next = remaining.next();
        let argument = match next {
            None => break,
            Some(value) => value.as_str(),
        };
        if is_help_flag(argument) {
            return Ok(ParsedArgs {
                source,
                target,
                action: CliAction::Help,
            });
        }
        if is_version_flag(argument) {
            return Ok(ParsedArgs {
                source,
                target,
                action: CliAction::Version,
            });
        }
        let is_source = argument == SOURCE_FLAG || argument == SOURCE_LONG_FLAG;
        if is_source {
            let value = parse_flag_value(&mut remaining, command_name)?;
            source = Some(value);
            continue;
        }
        if allow_target {
            let is_target =
                argument == TARGET_FLAG || argument == TARGET_LONG_FLAG;
            if is_target {
                let value = parse_flag_value(&mut remaining, command_name)?;
                target = Some(value);
                continue;
            }
        }
        return Err(unknown_argument(command_name, argument));
    }
    return Ok(ParsedArgs {
        source,
        target,
        action: CliAction::Run,
    });
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

/// Print the help text of a subcommand to stdout.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when stdout cannot be written.
pub fn print_help(help_text: &str) -> Result<(), Error> {
    return print_line(help_text);
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
