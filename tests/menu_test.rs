//! Integration tests for the start-menu layout helpers.

use vantools::tui::menu::{
    LOGO_ROWS, MIN_WIDTH, SLOGAN, build_frame, next_index, previous_index,
};

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
        assert!(line.len() <= limit, "line is too long: {line:?}");
    }
}

#[test]
fn frame_marks_only_the_selected_entry() {
    let frame = build_frame(1);
    assert!(frame.contains("> Verify"));
    assert!(!frame.contains("> Pack"));
    assert!(frame.contains(SLOGAN));
    for row in LOGO_ROWS {
        assert!(frame.contains(row));
    }
}
