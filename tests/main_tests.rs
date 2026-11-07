//! Integration tests for error rendering and exit-code mapping.

use vantools::common::Error;
use vantools::common::VanError;
use vantools::common::cli::{FALLBACK_EXIT_CODE, to_exit_code};

#[test]
fn every_error_variant_renders_with_the_error_prefix() {
    let usage = Error::Usage(String::from("missing subcommand"));
    assert_eq!(format!("{usage}"), "error: missing subcommand");
    let runtime = Error::Runtime(String::from("disk full"));
    assert_eq!(format!("{runtime}"), "error: disk full");
    let validation = Error::ValidationError(String::from("bad entry"));
    assert_eq!(format!("{validation}"), "error: bad entry");
    let cli_error = Error::CliError(String::from("unknown argument"));
    assert_eq!(format!("{cli_error}"), "error: unknown argument");
}

#[test]
fn usage_errors_exit_with_two_and_other_errors_with_one() {
    assert_eq!(Error::Usage(String::from("value")).exit_code(), 2);
    assert_eq!(Error::Runtime(String::from("value")).exit_code(), 1);
    assert_eq!(Error::ValidationError(String::from("value")).exit_code(), 1);
    assert_eq!(Error::CliError(String::from("value")).exit_code(), 1);
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
