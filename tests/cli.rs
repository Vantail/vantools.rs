//! Integration tests for the `van` CLI skeleton.

use std::process::Command;
use std::process::Output;

/// Marker contained in the shared help text.
const HELP_MARKER: &str = "Usage: van";

/// Expected message for an unknown subcommand.
const UNKNOWN_MESSAGE: &str = "error: unknown subcommand 'foo'";

/// Run the compiled `van` binary with the given arguments.
fn run(arguments: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_van"));
    let launched = command.args(arguments).output();
    return match launched {
        Ok(output) => output,
        Err(error) => panic!("failed to run the van binary: {error}"),
    };
}

#[test]
fn help_prints_usage_and_exits_zero() {
    let output = run(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(HELP_MARKER));
    assert!(stdout.contains("--version"));
}

#[test]
fn version_prints_name_and_version() {
    let output = run(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected: String = format!("van {}", env!("CARGO_PKG_VERSION"));
    assert!(stdout.contains(expected.as_str()));
}

#[test]
fn missing_subcommand_prints_help_and_exits_two() {
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(HELP_MARKER));
}

#[test]
fn unknown_subcommand_reports_usage_error() {
    let output = run(&["foo"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(UNKNOWN_MESSAGE));
}

#[cfg(unix)]
#[test]
fn non_unicode_argument_is_rejected_without_panicking() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let invalid: &OsStr = OsStr::from_bytes(&[0xff, 0xfe]);
    let mut command = Command::new(env!("CARGO_BIN_EXE_van"));
    let launched = command.arg(invalid).output();
    let output = match launched {
        Ok(output) => output,
        Err(error) => panic!("failed to run the van binary: {error}"),
    };
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not valid Unicode"));
}
