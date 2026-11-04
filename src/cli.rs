//! CLI support helpers for reporting errors and mapping exit codes.

use std::io::Write;

use crate::common::Error;

/// Exit code used when an error reports a value outside the `u8` range.
pub const FALLBACK_EXIT_CODE: u8 = 1;

/// Write an error diagnostic to the given stream.
///
/// Diagnostic output is best-effort. If writing fails, for example because
/// stderr is unavailable, the failure is ignored so the caller can still
/// return the error's exit code instead of panicking.
pub fn report_error<W>(stream: &mut W, error: &Error)
where
    W: Write,
{
    let written = writeln!(stream, "{error}");
    match written {
        Ok(()) => return,
        Err(_) => {
            // The diagnostic cannot be delivered; the exit code still
            // reports the failure.
            return;
        }
    }
}

/// Convert an error's exit code to a valid process exit code.
///
/// Values outside the `0..=255` range are mapped to a generic failure
/// code so a failed run is never reported as successful.
pub fn to_exit_code(code: i32) -> u8 {
    return match u8::try_from(code) {
        Ok(value) => value,
        Err(_) => FALLBACK_EXIT_CODE,
    };
}
