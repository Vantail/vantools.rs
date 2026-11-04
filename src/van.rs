//! Subcommand dispatcher for the `van` CLI.

use std::env;
use std::io;
use std::io::Write;

use crate::common::{Error, Result};

/// Help text shared by `--help` and the missing-subcommand path.
const HELP_TEXT: &str = "\
Usage: van [--help] [--version]

Options:
  --help     Show this help text
  --version  Show the version
";

/// Maximum number of argument characters echoed in a diagnostic.
pub const MAX_ECHO_CHARS: usize = 64;

/// Parse the process arguments and dispatch a subcommand.
///
/// Arguments that are not valid Unicode are rejected as usage errors
/// rather than aborting the process.
///
/// # Errors
///
/// Returns [`Error::Usage`] when the arguments are missing, unknown, or
/// not valid Unicode, and [`Error::Runtime`] when writing to the
/// terminal fails.
pub fn run() -> Result<()> {
    let mut arguments: Vec<String> = Vec::new();
    for raw_argument in env::args_os().skip(1) {
        let converted = raw_argument.into_string();
        let argument = match converted {
            Ok(value) => value,
            Err(_) => {
                return Err(Error::Usage(String::from(
                    "command-line argument is not valid Unicode",
                )));
            }
        };
        arguments.push(argument);
    }
    return run_with_args(arguments);
}

/// Parse the given arguments and dispatch a subcommand.
///
/// # Errors
///
/// Returns [`Error::Usage`] when the arguments are missing or unknown,
/// and [`Error::Runtime`] when writing to the terminal fails.
pub fn run_with_args<I>(arguments: I) -> Result<()>
where
    I: IntoIterator<Item = String>,
{
    let mut arguments = arguments.into_iter();
    let first = arguments.next();
    let argument = match first {
        None => {
            let written = write_stderr(HELP_TEXT);
            return match written {
                Ok(()) => Err(Error::Usage(String::from("missing subcommand"))),
                Err(error) => Err(error),
            };
        }
        Some(value) => value,
    };
    if argument == "--help" {
        let checked = reject_extra_arguments(arguments);
        return match checked {
            Ok(()) => write_stdout(HELP_TEXT),
            Err(error) => Err(error),
        };
    }
    if argument == "--version" {
        let checked = reject_extra_arguments(arguments);
        return match checked {
            Ok(()) => write_version(),
            Err(error) => Err(error),
        };
    }
    return Err(Error::Usage(format_unknown_subcommand(&argument)));
}

/// Reject any argument that follows `--help` or `--version`.
///
/// # Errors
///
/// Returns [`Error::Usage`] for the first extra argument.
fn reject_extra_arguments<I>(mut arguments: I) -> Result<()>
where
    I: Iterator<Item = String>,
{
    let extra = arguments.next();
    return match extra {
        None => Ok(()),
        Some(value) => Err(Error::Usage(format_unknown_subcommand(&value))),
    };
}

/// Render an unknown subcommand as a safe single-line diagnostic.
///
/// The argument is limited to [`MAX_ECHO_CHARS`] characters so that a very
/// long value cannot flood the terminal, and control characters are escaped
/// so that the message cannot inject terminal sequences or forge lines.
pub fn format_unknown_subcommand(argument: &str) -> String {
    let mut limited = String::new();
    for (index, character) in argument.chars().enumerate() {
        if index >= MAX_ECHO_CHARS {
            break;
        }
        limited.push(character);
    }
    let escaped = limited.escape_debug();
    return format!("unknown subcommand '{escaped}'");
}

/// Write the version banner to stdout.
fn write_version() -> Result<()> {
    let version: &str = env!("CARGO_PKG_VERSION");
    let line: String = format!("van {version}\n");
    return write_stdout(&line);
}

/// Write text to stdout.
fn write_stdout(text: &str) -> Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    return write_stream(&mut handle, text);
}

/// Write text to stderr.
fn write_stderr(text: &str) -> Result<()> {
    let stderr = io::stderr();
    let mut handle = stderr.lock();
    return write_stream(&mut handle, text);
}

/// Write text to a stream, converting an I/O failure into an error.
fn write_stream<W>(stream: &mut W, text: &str) -> Result<()>
where
    W: Write,
{
    let written = write!(stream, "{text}");
    return match written {
        Ok(()) => Ok(()),
        Err(error) => {
            Err(Error::Runtime(format!("failed to write output: {error}")))
        }
    };
}
