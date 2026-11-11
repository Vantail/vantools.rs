//! Integration tests for the form widget.
//!
//! Every test drives the widget through its public API only: the form is
//! built with `Form::new`, fed with constructed `InputEvent` values, and
//! rendered into an in-memory `LineWriter`.

use crossterm::event::{KeyCode, KeyModifiers};
use vantools::common::Error;
use vantools::tui::form::{Form, FormOutcome};
use vantools::tui::terminal::{InputEvent, LineWriter};

/// Build the segments of the reference form.
fn sample_segments() -> Vec<String> {
    return vec![
        String::from("van pack -s "),
        String::from(" -t "),
        String::from(""),
    ];
}

/// Build the field widths of the reference form.
fn sample_widths() -> Vec<usize> {
    return vec![4, 2];
}

/// Build the reference form, which always composes one line.
fn make_form() -> Form {
    let segments = sample_segments();
    let widths = sample_widths();
    let built = Form::new(&segments, &widths);
    return match built {
        Ok(form) => form,
        Err(error) => panic!("the sample form should build: {error}"),
    };
}

/// Build a key event without modifiers.
fn key(code: KeyCode) -> InputEvent {
    return key_with(code, KeyModifiers::NONE);
}

/// Build a key event with the given modifiers.
fn key_with(code: KeyCode, modifiers: KeyModifiers) -> InputEvent {
    return InputEvent::Key {
        code,
        modifiers,
    };
}

/// Return the rejection message of an invalid form, or `None` when the
/// form was accepted.
fn rejection_message(segments: &[String], widths: &[usize]) -> Option<String> {
    let built = Form::new(segments, widths);
    return match built {
        Ok(_) => None,
        Err(Error::Runtime(message)) => Some(message),
        Err(_) => panic!("the rejection should be a runtime error"),
    };
}

#[test]
fn new_rejects_empty_widths() {
    let segments = vec![String::from("only")];
    let widths: Vec<usize> = Vec::new();
    let message = rejection_message(&segments, &widths);
    let expected = Some(String::from("form requires at least one field"));
    assert_eq!(message, expected);
}

#[test]
fn new_rejects_a_segment_count_mismatch() {
    let segments = vec![String::from("a"), String::from("b")];
    let widths = vec![2, 2];
    let message = rejection_message(&segments, &widths);
    let expected = Some(String::from(
        "form requires exactly one more text segment than fields",
    ));
    assert_eq!(message, expected);
}

#[test]
fn new_rejects_a_segment_with_a_line_break() {
    let segments =
        vec![String::from("a\nb"), String::from(""), String::from("")];
    let widths = vec![2, 2];
    let message = rejection_message(&segments, &widths);
    let expected = Some(String::from("form text must not contain line breaks"));
    assert_eq!(message, expected);
}

#[test]
fn new_rejects_a_zero_width() {
    let segments = sample_segments();
    let widths = vec![2, 0];
    let message = rejection_message(&segments, &widths);
    let expected =
        Some(String::from("text field width must be greater than zero"));
    assert_eq!(message, expected);
}

#[test]
fn new_builds_the_form_with_the_first_field_focused() {
    let form = make_form();
    assert_eq!(form.focus(), 0);
    let values = form.values();
    let expected = vec![String::new(), String::new()];
    assert_eq!(values, expected);
}

#[test]
fn tab_advances_the_focus_and_wraps() {
    let mut form = make_form();
    let first = form.handle(key(KeyCode::Tab));
    assert_eq!(first, FormOutcome::Redraw);
    assert_eq!(form.focus(), 1);
    let second = form.handle(key(KeyCode::Tab));
    assert_eq!(second, FormOutcome::Redraw);
    assert_eq!(form.focus(), 0);
}

#[test]
fn back_tab_wraps_to_the_last_field() {
    let mut form = make_form();
    let outcome = form.handle(key(KeyCode::BackTab));
    assert_eq!(outcome, FormOutcome::Redraw);
    assert_eq!(form.focus(), 1);
}

#[test]
fn typing_routes_into_the_focused_field() {
    let mut form = make_form();
    form.handle(key(KeyCode::Char('a')));
    form.handle(key(KeyCode::Tab));
    form.handle(key(KeyCode::Char('b')));
    let values = form.values();
    let expected = vec![String::from("a"), String::from("b")];
    assert_eq!(values, expected);
}

#[test]
fn a_shifted_character_is_inserted() {
    let mut form = make_form();
    let outcome =
        form.handle(key_with(KeyCode::Char('A'), KeyModifiers::SHIFT));
    assert_eq!(outcome, FormOutcome::Redraw);
    let values = form.values();
    let expected = vec![String::from("A"), String::new()];
    assert_eq!(values, expected);
}

#[test]
fn a_control_character_is_ignored() {
    let mut form = make_form();
    let outcome =
        form.handle(key_with(KeyCode::Char('x'), KeyModifiers::CONTROL));
    assert_eq!(outcome, FormOutcome::Nothing);
    let values = form.values();
    let expected = vec![String::new(), String::new()];
    assert_eq!(values, expected);
}

#[test]
fn editing_keys_change_the_focused_field() {
    let mut form = make_form();
    assert_eq!(form.handle(key(KeyCode::Char('a'))), FormOutcome::Redraw);
    assert_eq!(form.handle(key(KeyCode::Char('b'))), FormOutcome::Redraw);
    assert_eq!(form.handle(key(KeyCode::Left)), FormOutcome::Redraw);
    assert_eq!(form.handle(key(KeyCode::Char('X'))), FormOutcome::Redraw);
    let values = form.values();
    let first = match values.first() {
        Some(value) => value,
        None => panic!("the form should keep two values"),
    };
    assert_eq!(first, &String::from("aXb"));
}

#[test]
fn enter_submits_without_changing_the_form() {
    let mut form = make_form();
    let first = form.handle(key(KeyCode::Enter));
    assert_eq!(first, FormOutcome::Submitted);
    let second = form.handle(key(KeyCode::Enter));
    assert_eq!(second, FormOutcome::Submitted);
    assert_eq!(form.focus(), 0);
    let values = form.values();
    let expected = vec![String::new(), String::new()];
    assert_eq!(values, expected);
}

#[test]
fn quit_cancels_without_changing_the_form() {
    let mut form = make_form();
    assert_eq!(form.handle(key(KeyCode::Char('a'))), FormOutcome::Redraw);
    assert_eq!(form.handle(key(KeyCode::Tab)), FormOutcome::Redraw);
    let outcome = form.handle(InputEvent::Quit);
    assert_eq!(outcome, FormOutcome::Cancelled);
    assert_eq!(form.focus(), 1);
    let values = form.values();
    let expected = vec![String::from("a"), String::new()];
    assert_eq!(values, expected);
}

#[test]
fn a_resize_changes_nothing() {
    let mut form = make_form();
    let outcome = form.handle(InputEvent::Resize {
        width: 80,
        height: 24,
    });
    assert_eq!(outcome, FormOutcome::Nothing);
    assert_eq!(form.focus(), 0);
}

#[test]
fn render_writes_one_line_and_the_cursor_follows_the_focus() {
    let mut form = make_form();
    form.handle(key(KeyCode::Char('a')));
    form.handle(key(KeyCode::Tab));
    form.handle(key(KeyCode::Char('b')));
    let mut writer = LineWriter::new(Vec::new());
    let written = form.render(&mut writer);
    assert!(written.is_ok());
    let actual = writer.into_inner();
    let expected = String::from("van pack -s [a...] -t [b.]\r\n").into_bytes();
    assert_eq!(actual, expected);
    let mut fresh = make_form();
    assert_eq!(fresh.cursor_column(), 13);
    fresh.handle(key(KeyCode::Tab));
    assert_eq!(fresh.cursor_column(), 23);
}
