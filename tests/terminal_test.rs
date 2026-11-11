//! Integration tests for the terminal foundation.
//!
//! The line writer is exercised in memory so the tests need no terminal,
//! and the input layer is exercised through the pure classifier.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use vantools::tui::terminal::InputEvent;
use vantools::tui::terminal::{LineWriter, classify};

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

#[test]
fn escape_press_classifies_as_quit() {
    let event = Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(classify(event), Some(InputEvent::Quit));
}

#[test]
fn control_c_classifies_as_quit() {
    let event =
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert_eq!(classify(event), Some(InputEvent::Quit));
}

#[test]
fn plain_c_classifies_as_key() {
    let event =
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
    let expected = InputEvent::Key {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(classify(event), Some(expected));
}

#[test]
fn arrow_key_classifies_as_key() {
    let event = Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    let expected = InputEvent::Key {
        code: KeyCode::Up,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(classify(event), Some(expected));
}

#[test]
fn escape_release_classifies_as_none() {
    let event = Event::Key(KeyEvent::new_with_kind(
        KeyCode::Esc,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));
    assert_eq!(classify(event), None);
}

#[test]
fn resize_classifies_as_resize() {
    let event = Event::Resize(80, 24);
    let expected = InputEvent::Resize {
        width: 80,
        height: 24,
    };
    assert_eq!(classify(event), Some(expected));
}

#[test]
fn focus_gained_classifies_as_none() {
    assert_eq!(classify(Event::FocusGained), None);
}
