//! Integration tests for the start-menu layout helpers.

use vantools::tui::menu::{
    LOGO_ROWS, MIN_WIDTH, SLOGAN, build_frame, next_index, previous_index,
};

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
fn down_moves_to_the_next_entry_and_wraps() {
    assert_eq!(next_index(0), 1);
    assert_eq!(next_index(3), 0);
}

#[test]
fn up_moves_to_the_previous_entry_and_wraps() {
    assert_eq!(previous_index(1), 0);
    assert_eq!(previous_index(0), 3);
}

#[test]
fn logo_rows_have_a_uniform_width() {
    let first = match LOGO_ROWS.first() {
        Some(value) => value.len(),
        None => return,
    };
    for row in LOGO_ROWS {
        assert_eq!(row.len(), first, "row is not uniform: {row:?}");
    }
}

#[test]
fn frame_uses_carriage_returns_between_lines() {
    let frame = build_frame(0);
    let mut previous = ' ';
    for character in frame.chars() {
        if character == '\n' {
            assert_eq!(previous, '\r', "line feed without a carriage return");
        }
        previous = character;
    }
}

#[test]
fn frame_fits_the_minimum_width() {
    let frame = build_frame(0);
    let limit = usize::from(MIN_WIDTH) - 1;
    for line in frame.lines() {
        let visible = visible_text(line);
        assert!(visible.len() <= limit, "line is too wide: {visible:?}");
    }
}

#[test]
fn frame_marks_only_the_selected_entry() {
    let frame = visible_text(&build_frame(1));
    assert!(frame.contains("> Verify"));
    assert!(!frame.contains("> Pack"));
    assert!(frame.contains(SLOGAN));
    for row in LOGO_ROWS {
        assert!(frame.contains(row), "logo row is missing: {row:?}");
    }
}

#[test]
fn logo_is_colored_and_resets_after_each_glyph() {
    let frame = build_frame(0);
    assert!(frame.contains("\x1b[31m_\x1b[0m"));
    assert!(frame.contains("\x1b[33m_\x1b[0m"));
    assert!(!frame.contains("\x1b[31m "), "a space was colored");
}

#[test]
fn frame_ends_with_the_italic_version_line() {
    let frame = build_frame(0);
    let expected =
        format!("\x1b[3mversion: {}\x1b[0m", env!("CARGO_PKG_VERSION"));
    assert!(frame.contains(&expected), "version line is missing");
}
