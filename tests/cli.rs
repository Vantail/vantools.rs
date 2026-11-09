//! Integration tests for the `van` CLI skeleton.

use std::process::Command;
use std::process::Output;

use vantools::common::validation::escape_path;

/// Marker contained in the shared help text.
const HELP_MARKER: &str = "Usage: van";

/// Expected message for an unknown subcommand.
const UNKNOWN_MESSAGE: &str = "error: unknown subcommand 'foo'";

/// Expected message for a `pack` invocation without source and target.
const MISSING_ARGS_MESSAGE: &str =
    "error: Missing required arguments. Run 'van pack --help' for usage.";

/// Count the lines of `text` with an explicit loop.
fn line_count(text: &str) -> usize {
    let mut count = 0;
    for _ in text.lines() {
        count += 1;
    }
    return count;
}

/// Create a temporary directory for one test.
fn temp_root() -> tempfile::TempDir {
    let created = tempfile::tempdir();
    return match created {
        Ok(value) => value,
        Err(error) => panic!("failed to create a temporary directory: {error}"),
    };
}

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
fn help_rejects_trailing_argument() {
    let output = run(&["--help", "--anything"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error: unknown subcommand '--anything'"));
}

#[test]
fn version_rejects_trailing_argument() {
    let output = run(&["--version", "foo"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(UNKNOWN_MESSAGE));
}

#[test]
fn no_arguments_prints_general_help_and_exits_zero() {
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(HELP_MARKER));
    assert!(stdout.contains("pack"));
    assert!(stdout.contains("verify"));
    assert!(stdout.contains("unpack"));
    assert!(output.stderr.is_empty());
}

#[test]
fn unknown_subcommand_reports_usage_error() {
    let output = run(&["foo"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(UNKNOWN_MESSAGE));
}

#[test]
fn unknown_subcommand_escapes_control_characters() {
    let output = run(&["evil\u{1b}[31m\nspoofed"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains('\u{1b}'));
    assert_eq!(line_count(&stderr), 1);
    assert!(stderr.contains("\\u{1b}"));
    assert!(stderr.contains("\\n"));
}

#[test]
fn unknown_subcommand_bounds_echoed_length() {
    let argument = "a".repeat(100);
    let output = run(&[argument.as_str()]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&"a".repeat(64)));
    assert!(!stderr.contains(&"a".repeat(65)));
}

#[test]
fn pack_help_prints_usage_and_exits_zero() {
    let long = run(&["pack", "--help"]);
    assert_eq!(long.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&long.stdout);
    assert!(stdout.contains("Usage: pack -s <source> -t <target>"));
    assert!(stdout.contains("--source"));
    let short = run(&["pack", "-h"]);
    assert_eq!(short.status.code(), Some(0));
    let short_stdout = String::from_utf8_lossy(&short.stdout);
    assert_eq!(short_stdout, stdout);
}

#[test]
fn pack_version_prints_name_and_version() {
    let output = run(&["pack", "-v"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected: String = format!("van {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(stdout, expected);
    let long = run(&["pack", "--version"]);
    assert_eq!(long.status.code(), Some(0));
    let long_stdout = String::from_utf8_lossy(&long.stdout);
    assert_eq!(long_stdout, expected);
}

#[test]
fn pack_unknown_argument_reports_cli_error() {
    let output = run(&["pack", "--bogus"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(
        "Unknown argument: --bogus. Run 'van pack --help' for usage."
    ));
}

#[test]
fn pack_unknown_argument_does_not_escape_backslashes() {
    let output = run(&["pack", "--bogus\\path"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(
        "Unknown argument: --bogus\\path. Run 'van pack --help' for usage."
    ));
}

#[test]
fn pack_unknown_argument_escapes_control_characters() {
    let output = run(&["pack", "bad\u{1b}[31m\nspoofed"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains('\u{1b}'));
    assert_eq!(line_count(&stderr), 1);
    assert!(stderr.contains("\\u{1b}"));
    assert!(stderr.contains("\\n"));
}

#[test]
fn pack_unknown_argument_bounds_echoed_length() {
    let argument = "a".repeat(100);
    let output = run(&["pack", argument.as_str()]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&"a".repeat(64)));
    assert!(stderr.contains("..."));
    assert!(!stderr.contains(&"a".repeat(65)));
}

#[test]
fn pack_missing_arguments_reports_cli_error() {
    let output = run(&["pack", "--source", "examples/test-plugin"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(MISSING_ARGS_MESSAGE));
}

#[test]
fn pack_accepts_long_flags() {
    let target = temp_root();
    let target_path = match target.path().to_str() {
        Some(value) => value,
        None => panic!("temporary directory path is not valid Unicode"),
    };
    let output = run(&[
        "pack",
        "--source",
        "examples/test-plugin",
        "--target",
        target_path,
    ]);
    assert_eq!(output.status.code(), Some(0));
    assert!(target.path().join("test-plugin-0.1.0.van").is_file());
}

#[test]
fn pack_accepts_short_flags() {
    let target = temp_root();
    let target_path = match target.path().to_str() {
        Some(value) => value,
        None => panic!("temporary directory path is not valid Unicode"),
    };
    let output =
        run(&["pack", "-s", "examples/test-plugin", "-t", target_path]);
    assert_eq!(output.status.code(), Some(0));
    assert!(target.path().join("test-plugin-0.1.0.van").is_file());
}

#[cfg(unix)]
#[test]
fn pack_escapes_control_characters_in_created_target_path() {
    let root = temp_root();
    let target = root.path().join("out\u{1b}[31m\nspoofed");
    let target_path = match target.to_str() {
        Some(value) => value,
        None => panic!("temporary directory path is not valid Unicode"),
    };
    let output =
        run(&["pack", "-s", "examples/test-plugin", "-t", target_path]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The control characters of the created directory name must be
    // rendered as escapes, never emitted raw or as an extra line.
    assert!(!stdout.contains('\u{1b}'));
    let expected =
        format!("created target directory: {}", escape_path(&target));
    assert!(stdout.contains(&expected), "unexpected output: {stdout}");
    assert!(target.join("test-plugin-0.1.0.van").is_file());
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
