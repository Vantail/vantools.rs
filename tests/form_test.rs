//! Integration tests for the interactive pack form helpers.

use vantools::tui::form::{FormState, build_frame, cursor_column};

/// Strip ANSI SGR sequences so layout checks measure visible text.
fn visible_text(text: &str) -> String {
    let mut visible = String::new();
    let mut in_escape = false;
    for character in text.chars() {
        if character == '\u{1b}' {
            in_escape = true;
            continue;
        }
        if in_escape {
            if character == 'm' {
                in_escape = false;
            }
            continue;
        }
        visible.push(character);
    }
    return visible;
}

#[test]
fn form_starts_on_the_source_field() {
    let state = FormState::new();
    assert_eq!(state.active_field(), 0);
    assert_eq!(state.source(), "");
    assert_eq!(state.target(), "");
    assert!(state.submit().is_none());
}

#[test]
fn typing_appends_to_the_active_field() {
    let mut state = FormState::new();
    state.push_char('s');
    state.push_char('r');
    state.push_char('c');
    assert_eq!(state.source(), "src");
    state.toggle_field();
    state.push_char('o');
    state.push_char('u');
    state.push_char('t');
    assert_eq!(state.target(), "out");
    assert_eq!(state.source(), "src");
}

#[test]
fn tab_switches_back_and_forth() {
    let mut state = FormState::new();
    state.toggle_field();
    assert_eq!(state.active_field(), 1);
    state.toggle_field();
    assert_eq!(state.active_field(), 0);
}

#[test]
fn backspace_removes_the_last_character_of_the_active_field() {
    let mut state = FormState::new();
    state.push_char('a');
    state.push_char('b');
    state.pop_char();
    assert_eq!(state.source(), "a");
    state.pop_char();
    state.pop_char();
    assert_eq!(state.source(), "");
    state.toggle_field();
    state.push_char('x');
    state.pop_char();
    assert_eq!(state.target(), "");
    assert_eq!(state.source(), "");
}

#[test]
fn submit_needs_both_fields() {
    let mut state = FormState::new();
    state.push_char('s');
    assert!(state.submit().is_none());
    state.toggle_field();
    state.push_char('t');
    let values = state.submit();
    assert_eq!(values, Some((String::from("s"), String::from("t"))));
}

#[test]
fn frame_shows_both_buffers_and_highlights_the_active_field() {
    let mut state = FormState::new();
    state.push_char('s');
    state.toggle_field();
    state.push_char('t');
    let frame = build_frame(&state);
    assert!(frame.contains("\x1b[1;32mt\x1b[0m"));
    assert!(!frame.contains("\x1b[1;32ms\x1b[0m"));
    assert_eq!(visible_text(&frame), "van pack --source [s] --target [t]");
}

#[test]
fn cursor_sits_after_the_active_buffer() {
    let mut state = FormState::new();
    state.push_char('s');
    state.push_char('r');
    assert_eq!(cursor_column(&state), 21);
    state.toggle_field();
    state.push_char('o');
    assert_eq!(cursor_column(&state), 34);
}
