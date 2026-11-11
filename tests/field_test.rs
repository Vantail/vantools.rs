//! Integration tests for the text field widget.
//!
//! Every test drives the widget through its public API only: the field
//! is built with `TextField::new`, edited with its insert, delete, and
//! cursor methods, and rendered into an in-memory `LineWriter`.

use crossterm::event::{KeyCode, KeyModifiers};
use vantools::common::Error;
use vantools::tui::field::TextField;
use vantools::tui::form::{Form, FormOutcome};
use vantools::tui::terminal::{InputEvent, LineWriter};

/// Build a field of the given width, which every test keeps valid.
fn make_field(width: usize) -> TextField {
    let built = TextField::new(width);
    return match built {
        Ok(field) => field,
        Err(error) => panic!("the field should build: {error}"),
    };
}

/// Insert every character of `text` in order.
fn insert_text(field: &mut TextField, text: &str) {
    for character in text.chars() {
        field.insert_char(character);
    }
}

/// Render one slot and return the bytes it wrote.
fn render_bytes(field: &TextField) -> Vec<u8> {
    let mut writer = LineWriter::new(Vec::new());
    let written = field.render_slot(&mut writer);
    assert!(written.is_ok());
    return writer.into_inner();
}

/// Build a one-field form for the delegation tests.
fn make_form() -> Form {
    let segments = vec![String::from("cmd "), String::from("")];
    let widths = vec![4];
    let built = Form::new(&segments, &widths);
    return match built {
        Ok(form) => form,
        Err(error) => panic!("the form should build: {error}"),
    };
}

/// Build a key event without modifiers.
fn key(code: KeyCode) -> InputEvent {
    return InputEvent::Key {
        code,
        modifiers: KeyModifiers::NONE,
    };
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
    insert_text(&mut field, "abc");
    assert_eq!(field.value(), String::from("abc"));
    assert_eq!(field.cursor_offset(), 4);
}

#[test]
fn inserting_at_the_cursor_places_the_character() {
    let mut field = make_field(5);
    insert_text(&mut field, "abc");
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
fn a_value_longer_than_the_slot_scrolls() {
    let mut field = make_field(5);
    insert_text(&mut field, "abcdef");
    assert_eq!(field.value(), String::from("abcdef"));
    assert_eq!(field.cursor_offset(), 5);
    let actual = render_bytes(&field);
    let expected = String::from("[cdef.]").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn appending_keeps_the_cursor_in_the_slot() {
    let mut field = make_field(5);
    insert_text(&mut field, "abcdefg");
    assert_eq!(field.value(), String::from("abcdefg"));
    assert_eq!(field.cursor_offset(), 5);
    let actual = render_bytes(&field);
    let expected = String::from("[defg.]").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn moving_left_scrolls_the_window_back() {
    let mut field = make_field(5);
    insert_text(&mut field, "abcdefg");
    let mut moves = 0;
    while moves < 6 {
        field.move_left();
        moves += 1;
    }
    assert_eq!(field.cursor_offset(), 1);
    let actual = render_bytes(&field);
    let expected = String::from("[bcdef]").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn deleting_at_the_window_start_keeps_it_visible() {
    let mut field = make_field(5);
    insert_text(&mut field, "abcdefg");
    let mut moves = 0;
    while moves < 6 {
        field.move_left();
        moves += 1;
    }
    field.delete();
    assert_eq!(field.value(), String::from("acdefg"));
    assert_eq!(field.cursor_offset(), 1);
    let actual = render_bytes(&field);
    let expected = String::from("[cdefg]").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn shrinking_below_the_width_resets_the_window() {
    let mut field = make_field(5);
    insert_text(&mut field, "abcdefg");
    field.backspace();
    field.backspace();
    field.backspace();
    assert_eq!(field.value(), String::from("abcd"));
    assert_eq!(field.cursor_offset(), 5);
    let actual = render_bytes(&field);
    let expected = String::from("[abcd.]").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn appending_eight_characters_keeps_the_cursor_in_the_slot() {
    let mut field = make_field(5);
    insert_text(&mut field, "abcdefgh");
    assert_eq!(field.value(), String::from("abcdefgh"));
    assert_eq!(field.cursor_offset(), 5);
}

#[test]
fn multi_byte_characters_scroll_by_character() {
    let mut field = make_field(5);
    insert_text(&mut field, "ääääää");
    assert_eq!(field.value(), String::from("ääääää"));
    let actual = render_bytes(&field);
    let expected = String::from("[ääää.]").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn backspace_removes_a_multi_byte_character() {
    let mut field = make_field(5);
    field.insert_char('ä');
    assert_eq!(field.cursor_offset(), 2);
    field.move_left();
    assert_eq!(field.cursor_offset(), 1);
    field.move_right();
    field.backspace();
    assert!(field.is_empty());
    assert_eq!(field.cursor_offset(), 1);
}

#[test]
fn a_fresh_field_renders_a_slot_full_of_padding() {
    let field = make_field(5);
    assert_eq!(field.cursor_offset(), 1);
    let actual = render_bytes(&field);
    let expected = String::from("[.....]").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn the_window_keeps_the_cursor_visible_after_an_insertion() {
    let mut field = make_field(5);
    insert_text(&mut field, "abcdef");
    field.move_left();
    field.move_left();
    field.move_left();
    field.insert_char('X');
    assert_eq!(field.value(), String::from("abcXdef"));
    assert_eq!(field.cursor_offset(), 3);
    let actual = render_bytes(&field);
    let expected = String::from("[cXdef]").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn an_escape_character_is_rejected() {
    let mut field = make_field(5);
    field.insert_char('\u{1b}');
    assert!(field.value().is_empty());
    assert_eq!(field.cursor_offset(), 1);
}

#[test]
fn line_breaks_are_rejected() {
    let mut field = make_field(5);
    field.insert_char('\n');
    field.insert_char('\r');
    assert!(field.value().is_empty());
    assert_eq!(field.cursor_offset(), 1);
}

#[test]
fn delete_and_c1_characters_are_rejected() {
    let mut field = make_field(5);
    field.insert_char('\u{7f}');
    field.insert_char('\u{9b}');
    assert!(field.value().is_empty());
    assert_eq!(field.cursor_offset(), 1);
}

#[test]
fn direction_overrides_are_rejected() {
    let mut field = make_field(5);
    field.insert_char('\u{202e}');
    field.insert_char('\u{2066}');
    assert!(field.value().is_empty());
    assert_eq!(field.cursor_offset(), 1);
}

#[test]
fn zero_width_characters_are_rejected() {
    let mut field = make_field(5);
    field.insert_char('\u{200b}');
    field.insert_char('\u{200f}');
    assert!(field.value().is_empty());
    assert_eq!(field.cursor_offset(), 1);
}

#[test]
fn rejected_characters_leave_no_gap() {
    let mut field = make_field(5);
    field.insert_char('a');
    field.insert_char('\u{1b}');
    field.insert_char('b');
    assert_eq!(field.value(), String::from("ab"));
    assert_eq!(field.cursor_offset(), 3);
}

#[test]
fn printable_characters_are_stored_verbatim() {
    let mut field = make_field(8);
    insert_text(&mut field, "ä -./");
    assert_eq!(field.value(), String::from("ä -./"));
    assert!(!field.is_empty());
}

#[test]
fn rejection_does_not_move_the_cursor() {
    let mut filtered = make_field(5);
    filtered.insert_char('a');
    filtered.insert_char('\u{202e}');
    filtered.insert_char('b');
    let mut direct = make_field(5);
    direct.insert_char('a');
    direct.insert_char('b');
    assert_eq!(filtered.value(), direct.value());
    assert_eq!(filtered.cursor_offset(), direct.cursor_offset());
}

#[test]
fn a_form_ignores_a_rejected_character() {
    let mut form = make_form();
    let outcome = form.handle(key(KeyCode::Char('\u{202e}')));
    assert_eq!(outcome, FormOutcome::Redraw);
    let values = form.values();
    let expected = vec![String::new()];
    assert_eq!(values, expected);
}

#[test]
fn a_form_inserts_a_dash_into_the_field() {
    let mut form = make_form();
    let outcome = form.handle(key(KeyCode::Char('-')));
    assert_eq!(outcome, FormOutcome::Redraw);
    let values = form.values();
    let expected = vec![String::from("-")];
    assert_eq!(values, expected);
}
