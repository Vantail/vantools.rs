//! Integration tests for the subcommand dispatcher helpers.

use vantools::van::{MAX_ECHO_CHARS, format_unknown_subcommand};

#[test]
fn unknown_subcommand_escapes_control_characters() {
    let rendered = format_unknown_subcommand("bad\u{1b}[31m\nspoofed");
    assert!(!rendered.contains('\u{1b}'));
    assert!(!rendered.contains('\n'));
    assert!(rendered.contains("\\u{1b}"));
    assert!(rendered.contains("\\n"));
}

#[test]
fn unknown_subcommand_limits_argument_length() {
    let argument = "a".repeat(MAX_ECHO_CHARS + 10);
    let rendered = format_unknown_subcommand(&argument);
    let expected =
        format!("unknown subcommand '{}'", "a".repeat(MAX_ECHO_CHARS));
    assert_eq!(rendered, expected);
}
