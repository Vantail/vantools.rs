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
fn frame_places_the_version_under_the_logo() {
    let frame = build_frame(0);
    let label = format!("version: {}", env!("CARGO_PKG_VERSION"));
    let styled = format!("\x1b[3m{label}\x1b[0m");
    let mut lines: Vec<&str> = Vec::new();
    for line in frame.lines() {
        lines.push(line);
    }
    let mut last_logo: Option<usize> = None;
    let mut index: usize = 0;
    while index < lines.len() {
        let line = match lines.get(index) {
            Some(value) => *value,
            None => break,
        };
        let visible = visible_text(line);
        let mut is_logo = false;
        for row in LOGO_ROWS {
            if visible.as_str() == row {
                is_logo = true;
            }
        }
        if is_logo {
            last_logo = Some(index);
        }
        index += 1;
    }
    let logo_index = match last_logo {
        Some(value) => value,
        None => panic!("logo rows are missing"),
    };
    let version_index = logo_index + 1;
    let version_line = match lines.get(version_index) {
        Some(value) => *value,
        None => panic!("version line is missing"),
    };
    assert!(version_line.contains(&styled), "version is not italic");
    let mut max_width: usize = 0;
    for row in LOGO_ROWS {
        if row.len() > max_width {
            max_width = row.len();
        }
    }
    let visible = visible_text(version_line);
    assert_eq!(visible.len(), max_width, "version is not right-aligned");
    let slogan_index = version_index + 2;
    let slogan_line = match lines.get(slogan_index) {
        Some(value) => *value,
        None => panic!("slogan line is missing"),
    };
    assert_eq!(visible_text(slogan_line), SLOGAN, "slogan is not below");
}
