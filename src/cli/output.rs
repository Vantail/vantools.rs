//! Terminal output helpers for the `van` binary.
//!
//! Writing is explicit: a stream that cannot be written produces an error
//! instead of a panic, and `failure` names the operation in that error so
//! the message stays specific to the caller.

use std::io;
use std::io::Write;

use vantools::common::Error;

/// General help text printed when `van` runs without arguments on a
/// non-interactive stream.
const GENERAL_HELP: &str = "\
Usage: van <command> [options]

Commands:
  pack    Compress a plugin directory into an archive
  verify  Validate an archive
  unpack  Extract an archive

Run 'van <command> --help' for command-specific help.
";

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

/// Print the logo and the general help text to stdout.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when stdout cannot be written.
pub fn print_general_help() -> Result<(), Error> {
    vantools::tui::logo::print_logo();
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
