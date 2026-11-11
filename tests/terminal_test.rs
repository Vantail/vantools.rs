//! Integration tests for the carriage-return-safe line writer.

use vantools::tui::terminal::LineWriter;

#[test]
fn write_line_appends_carriage_return_and_newline() {
    let mut writer = LineWriter::new(Vec::new());
    let written = writer.write_line("hello");
    assert!(written.is_ok());
    let actual = writer.into_inner();
    let expected = String::from("hello\r\n").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn write_line_normalizes_embedded_newlines() {
    let mut writer = LineWriter::new(Vec::new());
    let written = writer.write_line("a\nb");
    assert!(written.is_ok());
    let actual = writer.into_inner();
    let expected = String::from("a\r\nb\r\n").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn write_block_terminates_every_line() {
    let mut writer = LineWriter::new(Vec::new());
    let written = writer.write_block("a\nb\nc");
    assert!(written.is_ok());
    let actual = writer.into_inner();
    let expected = String::from("a\r\nb\r\nc\r\n").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn write_block_of_empty_text_writes_nothing() {
    let mut writer = LineWriter::new(Vec::new());
    let written = writer.write_block("");
    assert!(written.is_ok());
    let actual = writer.into_inner();
    let expected: Vec<u8> = Vec::new();
    assert_eq!(actual, expected);
}

#[test]
fn write_line_of_empty_text_writes_one_terminator() {
    let mut writer = LineWriter::new(Vec::new());
    let written = writer.write_line("");
    assert!(written.is_ok());
    let actual = writer.into_inner();
    let expected = String::from("\r\n").into_bytes();
    assert_eq!(actual, expected);
}
