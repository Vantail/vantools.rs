//! Subcommand dispatcher for the `van` CLI.

use std::io;
use std::io::Write;

use crate::common::cli::{collect_arguments, version_line};
use crate::common::validation::escape_text;
use crate::common::{Error, Result};

/// Help text shared by `--help` and the missing-subcommand path.
const HELP_TEXT: &str = "\
Usage: van [--help] [--version]

Options:
  --help     Show this help text
  --version  Show the version
";

/// Maximum number of argument characters echoed in a diagnostic.
pub use crate::common::validation::MAX_ECHO_CHARS;

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
    let arguments = collect_arguments()?;
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
            write_stderr(HELP_TEXT)?;
            return Err(Error::Usage(String::from("missing subcommand")));
        }
        Some(value) => value,
    };
    if argument == "--help" {
        reject_extra_arguments(arguments)?;
        return write_stdout(HELP_TEXT);
    }
    if argument == "--version" {
        reject_extra_arguments(arguments)?;
        return write_version();
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
/// The argument is rendered with [`escape_text`]: it is limited to
/// [`MAX_ECHO_CHARS`] characters so that a very long value cannot flood
/// the terminal, and control characters are escaped so that the message
/// cannot inject terminal sequences or forge lines.
pub fn format_unknown_subcommand(argument: &str) -> String {
    let rendered = escape_text(argument);
    return format!("unknown subcommand '{rendered}'");
}

/// Write the version banner to stdout.
fn write_version() -> Result<()> {
    let line: String = version_line();
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let written = writeln!(handle, "{line}");
    return map_write_failure(written);
}

/// Write text to stdout.
fn write_stdout(text: &str) -> Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let written = write!(handle, "{text}");
    return map_write_failure(written);
}

/// Write text to stderr.
fn write_stderr(text: &str) -> Result<()> {
    let stderr = io::stderr();
    let mut handle = stderr.lock();
    let written = write!(handle, "{text}");
    return map_write_failure(written);
}

/// Convert a write result into the error reported for a failed write.
///
/// The shared output helpers moved to the `van` binary, so this module
/// keeps the few writes its own help and version output needs.
fn map_write_failure(written: io::Result<()>) -> Result<()> {
    return match written {
        Ok(()) => Ok(()),
        Err(error) => {
            Err(Error::Runtime(format!("failed to write output: {error}")))
        }
    };
}
