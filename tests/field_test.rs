//! Integration tests for the text field widget.
//!
//! Every test drives the widget through its public API only: the field
//! is built with `TextField::new`, edited with its insert, delete, and
//! cursor methods, and rendered into an in-memory `LineWriter`.

use vantools::common::Error;
use vantools::tui::field::TextField;
use vantools::tui::terminal::LineWriter;

/// Build a field of the given width, which every test keeps valid.
fn make_field(width: usize) -> TextField {
    let built = TextField::new(width);
    return match built {
        Ok(field) => field,
        Err(error) => panic!("the field should build: {error}"),
    };
}

/// Render one slot and return the bytes it wrote.
fn render_bytes(field: &TextField) -> Vec<u8> {
    let mut writer = LineWriter::new(Vec::new());
    let written = field.render_slot(&mut writer);
    assert!(written.is_ok());
    return writer.into_inner();
}

#[test]
fn write_text_writes_no_trailing_newline() {
    let mut writer = LineWriter::new(Vec::new());
    let written = writer.write_text("abc");
    assert!(written.is_ok());
    let actual = writer.into_inner();
    let expected = String::from("abc").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn write_text_normalizes_embedded_newlines() {
    let mut writer = LineWriter::new(Vec::new());
    let written = writer.write_text("a\nb");
    assert!(written.is_ok());
    let actual = writer.into_inner();
    let expected = String::from("a\r\nb").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn write_text_then_write_line_shares_the_buffer() {
    let mut writer = LineWriter::new(Vec::new());
    let text_written = writer.write_text("abc");
    assert!(text_written.is_ok());
    let line_written = writer.write_line("d");
    assert!(line_written.is_ok());
    let actual = writer.into_inner();
    let expected = String::from("abcd\r\n").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn new_rejects_a_zero_width() {
    let built = TextField::new(0);
    let message = match built {
        Ok(_) => panic!("a zero width should be rejected"),
        Err(Error::Runtime(text)) => text,
        Err(_) => panic!("the error should be a runtime error"),
    };
    let expected = String::from("text field width must be greater than zero");
    assert_eq!(message, expected);
}

#[test]
fn a_fresh_field_is_empty() {
    let field = make_field(5);
    assert!(field.value().is_empty());
    assert!(field.is_empty());
    assert_eq!(field.cursor_offset(), 1);
}

#[test]
fn inserting_characters_advances_the_cursor() {
    let mut field = make_field(5);
    field.insert_char('a');
    field.insert_char('b');
    field.insert_char('c');
    assert_eq!(field.value(), String::from("abc"));
    assert_eq!(field.cursor_offset(), 4);
}

#[test]
fn inserting_into_a_full_field_changes_nothing() {
    let mut field = make_field(3);
    field.insert_char('a');
    field.insert_char('b');
    field.insert_char('c');
    field.insert_char('d');
    assert_eq!(field.value(), String::from("abc"));
    assert_eq!(field.cursor_offset(), 4);
}

#[test]
fn inserting_at_the_cursor_places_the_character() {
    let mut field = make_field(5);
    field.insert_char('a');
    field.insert_char('b');
    field.insert_char('c');
    field.move_left();
    field.insert_char('X');
    assert_eq!(field.value(), String::from("abXc"));
    assert_eq!(field.cursor_offset(), 4);
}

#[test]
fn backspace_removes_the_character_before_the_cursor() {
    let mut field = make_field(5);
    field.backspace();
    assert!(field.is_empty());
    assert_eq!(field.cursor_offset(), 1);
    field.insert_char('a');
    field.insert_char('b');
    field.backspace();
    assert_eq!(field.value(), String::from("a"));
    assert_eq!(field.cursor_offset(), 2);
}

#[test]
fn delete_removes_the_character_at_the_cursor() {
    let mut field = make_field(5);
    field.insert_char('a');
    field.insert_char('b');
    field.delete();
    assert_eq!(field.value(), String::from("ab"));
    field.move_left();
    field.delete();
    assert_eq!(field.value(), String::from("a"));
}

#[test]
fn cursor_moves_stop_at_the_boundaries() {
    let mut field = make_field(5);
    field.move_left();
    assert_eq!(field.cursor_offset(), 1);
    field.insert_char('a');
    field.move_right();
    assert_eq!(field.cursor_offset(), 2);
    field.move_right();
    assert_eq!(field.cursor_offset(), 2);
}

#[test]
fn render_slot_pads_an_empty_field() {
    let field = make_field(5);
    let actual = render_bytes(&field);
    let expected = String::from("[.....]").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn render_slot_pads_a_short_value() {
    let mut field = make_field(5);
    field.insert_char('a');
    field.insert_char('b');
    let actual = render_bytes(&field);
    let expected = String::from("[ab...]").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn cursor_offset_tracks_the_edit_position() {
    let mut field = make_field(5);
    field.insert_char('a');
    field.insert_char('b');
    field.move_left();
    assert_eq!(field.cursor_offset(), 2);
}

#[test]
fn backspace_removes_a_multi_byte_character() {
    let mut field = make_field(5);
    field.insert_char('ä');
    // The cursor counts characters, so one multi-byte character leaves
    // it one past the opening bracket of the slot.
    assert_eq!(field.cursor_offset(), 2);
    field.move_left();
    assert_eq!(field.cursor_offset(), 1);
    field.move_right();
    field.backspace();
    assert!(field.is_empty());
    assert_eq!(field.cursor_offset(), 1);
}

#[test]
fn render_slot_pads_by_characters_not_bytes() {
    let mut field = make_field(4);
    field.insert_char('ä');
    field.insert_char('b');
    let actual = render_bytes(&field);
    let expected = String::from("[äb..]").into_bytes();
    assert_eq!(actual, expected);
}
