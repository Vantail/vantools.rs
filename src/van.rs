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

/// Parse the process arguments and dispatch a subcommand.
///
/// # Errors
///
/// Returns [`Error::Usage`] when the arguments are missing or unknown,
/// and [`Error::Runtime`] when writing to the terminal fails.
pub fn run() -> Result<()> {
    return run_with_args(env::args().skip(1));
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
        return write_stdout(HELP_TEXT);
    }
    if argument == "--version" {
        return write_version();
    }
    return Err(Error::Usage(format!("unknown subcommand '{argument}'")));
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
