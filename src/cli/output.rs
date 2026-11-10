//! Terminal output helpers for the `van` binary.
//!
//! Every line the binary prints is plain text: no colors, no timestamps,
//! and no terminal detection. Writing is explicit where a caller has to
//! report a failure, and best effort for the log helpers, because a
//! stream that cannot be written cannot report its own failure either.

use std::io;
use std::io::Write;

use vantools::common::Error;

/// General help text printed when `van` runs without arguments.
const GENERAL_HELP: &str = "\
Usage: van [options]
Commands:
pack    Compress a plugin directory into an archive
verify  Validate an archive
unpack  Extract an archive
Run 'van <command> --help' for command-specific help.
";

/// Destination stream of one log line.
enum Stream {
    /// Standard output.
    Output,
    /// Standard error.
    Error,
}

/// Write an informational line to stdout.
pub fn log_info(message: &str, value: &str) {
    log_line(Stream::Output, message, value);
}

/// Write a success line to stdout.
pub fn log_ok(message: &str, value: &str) {
    log_line(Stream::Output, message, value);
}

/// Write a warning line to stderr.
pub fn log_warn(message: &str, value: &str) {
    log_line(Stream::Error, message, value);
}

/// Write an error line to stderr.
pub fn log_err(message: &str, value: &str) {
    log_line(Stream::Error, message, value);
}

/// Write a start line to stdout.
///
/// A start line announces an operation rather than reporting a result,
/// so it carries no value.
pub fn log_start(message: &str) {
    log_line(Stream::Output, message, "");
}

/// Write a multi-line text block to stdout, one line per write.
///
/// A block is a code-like body rather than a log event, so the text is
/// written unchanged.
pub fn log_block(text: &str) {
    let stdout = io::stdout();
    let mut locked = stdout.lock();
    for line in text.lines() {
        let written = writeln!(locked, "{line}");
        if written.is_err() {
            return;
        }
    }
    return;
}

/// Write one plain log line to `stream`.
///
/// The value is appended only when there is one, so a line that
/// announces an operation does not end with a dangling separator.
fn log_line(stream: Stream, message: &str, value: &str) {
    let line = plain_line(message, value);
    let written = match stream {
        Stream::Output => {
            let handle = io::stdout();
            let mut locked = handle.lock();
            writeln!(locked, "{line}")
        }
        Stream::Error => {
            let handle = io::stderr();
            let mut locked = handle.lock();
            writeln!(locked, "{line}")
        }
    };
    if written.is_err() {
        // The stream that failed cannot report its own failure.
        return;
    }
    return;
}

/// Build the plain `Message: Value` line, or `Message` without a value.
fn plain_line(message: &str, value: &str) -> String {
    if value.is_empty() {
        return String::from(message);
    }
    return format!("{message}: {value}");
}

/// Write `text` and a trailing newline to `stream`.
///
/// `failure` names the attempted operation, for example `output` or
/// `warning`.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the stream cannot be written.
pub fn write_line<W>(
    stream: &mut W,
    text: &str,
    failure: &str,
) -> Result<(), Error>
where
    W: Write,
{
    let written = writeln!(stream, "{text}");
    return map_failure(written, failure);
}

/// Print the general help text to stdout.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when stdout cannot be written.
pub fn print_general_help() -> Result<(), Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let written = write!(handle, "{GENERAL_HELP}");
    return map_failure(written, "output");
}

/// Convert a write result into an error that names the failed operation.
fn map_failure(written: io::Result<()>, failure: &str) -> Result<(), Error> {
    return match written {
        Ok(()) => Ok(()),
        Err(error) => {
            Err(Error::Runtime(format!("failed to write {failure}: {error}")))
        }
    };
}
