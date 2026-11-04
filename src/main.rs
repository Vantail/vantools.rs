//! CLI entry point for the `van` package and plugin manager.

use std::io;
use std::io::Write;
use std::process::ExitCode;

use vantools::common::{Error, VanError};
use vantools::van;

/// Exit code used when an error reports a value outside the `u8` range.
const FALLBACK_EXIT_CODE: u8 = 1;

fn main() -> ExitCode {
    let result = van::run();
    let error = match result {
        Ok(()) => {
            return ExitCode::SUCCESS;
        }
        Err(error) => error,
    };
    let stderr = io::stderr();
    let mut handle = stderr.lock();
    report_error(&mut handle, &error);
    let code: i32 = error.exit_code();
    return ExitCode::from(to_exit_code(code));
}

/// Write an error diagnostic to the given stream.
///
/// Diagnostic output is best-effort. If writing fails, for example because
/// stderr is unavailable, the failure is ignored so the caller can still
/// return the error's exit code instead of panicking.
fn report_error<W>(stream: &mut W, error: &Error)
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
fn to_exit_code(code: i32) -> u8 {
    return match u8::try_from(code) {
        Ok(value) => value,
        Err(_) => FALLBACK_EXIT_CODE,
    };
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::io::Write;

    use super::{FALLBACK_EXIT_CODE, report_error, to_exit_code};
    use vantools::common::Error;

    /// Writer that always fails, simulating an unavailable stderr.
    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            return Err(io::Error::other("stderr unavailable"));
        }

        fn flush(&mut self) -> io::Result<()> {
            return Err(io::Error::other("stderr unavailable"));
        }
    }

    #[test]
    fn error_reporting_ignores_write_failures() {
        let error = Error::Usage(String::from("missing subcommand"));
        let mut writer = FailingWriter;
        report_error(&mut writer, &error);
    }

    #[test]
    fn in_range_exit_codes_are_preserved() {
        assert_eq!(to_exit_code(0), 0);
        assert_eq!(to_exit_code(1), 1);
        assert_eq!(to_exit_code(255), 255);
    }

    #[test]
    fn out_of_range_exit_codes_map_to_failure() {
        assert_eq!(to_exit_code(-1), FALLBACK_EXIT_CODE);
        assert_eq!(to_exit_code(256), FALLBACK_EXIT_CODE);
        assert_eq!(to_exit_code(i32::MAX), FALLBACK_EXIT_CODE);
    }
}
