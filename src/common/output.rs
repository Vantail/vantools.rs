//! Terminal output helpers shared by the commands.
//!
//! Writing is explicit: a stream that cannot be written produces an error
//! instead of a panic, and `failure` names the operation in that error so
//! the message stays specific to the caller.

use std::io;
use std::io::Write;

use crate::common::Error;

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

/// Write `text` verbatim to `stream`.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the stream cannot be written.
pub fn write_text<W>(
    stream: &mut W,
    text: &str,
    failure: &str,
) -> Result<(), Error>
where
    W: Write,
{
    let written = write!(stream, "{text}");
    return map_failure(written, failure);
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
