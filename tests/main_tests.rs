//! Integration tests for the CLI error-reporting helpers.

use std::io;
use std::io::Write;

use vantools::cli::{FALLBACK_EXIT_CODE, report_error, to_exit_code};
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
