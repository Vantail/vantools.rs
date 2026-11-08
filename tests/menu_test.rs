//! Integration tests for the start-menu layout and state machine.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use vantools::tui::menu::{
    AppState, LOGO_ROWS, MIN_WIDTH, MenuChoice, MenuSelection, SLOGAN,
    build_frame, handle_key, next_index, previous_index,
};

/// Hint shown while the command list is active.
const MENU_HINT: &str = "↑↓ Select | Enter Confirm | Q Quit";

/// Hint shown while a parameter form is active.
const FORM_HINT: &str = "Tab to switch | Enter to confirm | Esc to cancel";

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

/// Return the rendered frame of `state`.
fn frame_of(state: &AppState) -> String {
    let (frame, _cursor) = build_frame(state);
    return frame;
}

/// Return the rendered frame of `state` without its attributes.
fn visible_frame_of(state: &AppState) -> String {
    return visible_text(&frame_of(state));
}

/// Press one key without modifiers.
fn press(state: &mut AppState, code: KeyCode) -> Option<MenuSelection> {
    let key = KeyEvent::new(code, KeyModifiers::NONE);
    return handle_key(state, &key);
}

/// Type `text` into the active form field.
fn type_text(state: &mut AppState, text: &str) {
    for character in text.chars() {
        let finished = press(state, KeyCode::Char(character));
        assert!(finished.is_none(), "typing finished the menu early");
    }
}

/// Return the separator drawn above a form.
fn separator() -> String {
    return "-".repeat(50);
}

/// Report whether `row` holds anything other than whitespace.
fn row_has_glyph(row: &str) -> bool {
    for character in row.chars() {
        if !character.is_whitespace() {
            return true;
        }
    }
    return false;
}

/// Return the lines of `frame` in order.
fn frame_lines(frame: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = Vec::new();
    for line in frame.lines() {
        lines.push(line);
    }
    return lines;
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
    let states =
        [AppState::menu(0), AppState::pack_form(), AppState::verify_form()];
    for state in states {
        let frame = frame_of(&state);
        let mut previous = ' ';
        for character in frame.chars() {
            if character == '\n' {
                assert_eq!(
                    previous, '\r',
                    "line feed without a carriage return"
                );
            }
            previous = character;
        }
    }
}

#[test]
fn frame_fits_the_minimum_width() {
    let states =
        [AppState::menu(0), AppState::pack_form(), AppState::verify_form()];
    let limit = usize::from(MIN_WIDTH) - 1;
    for state in states {
        let frame = frame_of(&state);
        for line in frame.lines() {
            let visible = visible_text(line);
            assert!(visible.len() <= limit, "line is too wide: {visible:?}");
        }
    }
}

#[test]
fn frame_marks_only_the_selected_entry() {
    let frame = visible_frame_of(&AppState::menu(1));
    assert!(frame.contains("> Verify"));
    assert!(!frame.contains("> Pack"));
    assert!(frame.contains(SLOGAN));
    for row in LOGO_ROWS {
        if !row_has_glyph(row) {
            continue;
        }
        assert!(frame.contains(row), "logo row is missing: {row:?}");
    }
}

#[test]
fn logo_is_colored_and_resets_after_each_glyph() {
    let frame = frame_of(&AppState::menu(0));
    assert!(frame.contains("\x1b[31m_\x1b[0m"));
    assert!(frame.contains("\x1b[33m_\x1b[0m"));
    assert!(!frame.contains("\x1b[31m "), "a space was colored");
}

#[test]
fn frame_places_the_version_under_the_logo() {
    let frame = frame_of(&AppState::menu(0));
    let label = format!("version: {}", env!("CARGO_PKG_VERSION"));
    let styled = format!("\x1b[3m{label}\x1b[0m");
    let lines = frame_lines(&frame);
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
            if row_has_glyph(row) && visible.as_str() == row {
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

#[test]
fn form_keeps_the_logo_and_the_menu_on_screen() {
    let mut state = AppState::pack_form();
    type_text(&mut state, "src");
    press(&mut state, KeyCode::Tab);
    type_text(&mut state, "out");
    let frame = visible_frame_of(&state);
    assert!(frame.contains(SLOGAN), "slogan disappeared");
    assert!(frame.contains("> Pack"), "selection disappeared");
    let version = format!("version: {}", env!("CARGO_PKG_VERSION"));
    assert!(frame.contains(&version), "version disappeared");
    let mut logo_rows: usize = 0;
    for row in LOGO_ROWS {
        if row_has_glyph(row) {
            assert!(frame.contains(row), "logo row is missing: {row:?}");
            logo_rows += 1;
        }
    }
    assert!(logo_rows > 0, "logo rows are missing");
    assert!(frame.contains(&separator()), "separator is missing");
    assert!(
        frame.contains("van pack --source [src] --target [out]"),
        "form line is missing: {frame}"
    );
}

#[test]
fn hint_matches_the_current_screen() {
    let (menu, menu_cursor) = build_frame(&AppState::menu(0));
    assert!(menu_cursor.is_none(), "the menu shows a cursor");
    let menu_visible = visible_text(&menu);
    assert!(menu_visible.contains(MENU_HINT));
    assert!(!menu_visible.contains(FORM_HINT));

    let (form, form_cursor) = build_frame(&AppState::pack_form());
    assert!(form_cursor.is_some(), "the form hides the cursor");
    let form_visible = visible_text(&form);
    assert!(form_visible.contains(FORM_HINT));
    assert!(!form_visible.contains(MENU_HINT));
    let mut hint_lines: usize = 0;
    for line in form_visible.lines() {
        if line.contains("Esc to cancel") {
            hint_lines += 1;
        }
    }
    assert_eq!(hint_lines, 1, "the hint line is duplicated");
}

#[test]
fn cursor_sits_after_the_active_source_text() {
    let mut state = AppState::pack_form();
    type_text(&mut state, "src");
    let (frame, cursor) = build_frame(&state);
    let position = match cursor {
        Some(value) => value,
        None => panic!("the form has no cursor"),
    };
    let lines = frame_lines(&frame);
    let mut form_row: u16 = 0;
    let mut row: u16 = 0;
    while row < lines.len() as u16 {
        let line = match lines.get(usize::from(row)) {
            Some(value) => *value,
            None => break,
        };
        if visible_text(line).contains("van pack --source") {
            form_row = row;
        }
        row += 1;
    }
    assert_eq!(position.1, form_row, "cursor is not on the form line");
    let line = match lines.get(usize::from(form_row)) {
        Some(value) => *value,
        None => panic!("form line is missing"),
    };
    let visible = visible_text(line);
    let prefix = String::from("van pack --source [src");
    assert!(visible.starts_with(&prefix));
    assert_eq!(usize::from(position.0), prefix.len());
}

#[test]
fn cursor_moves_with_typing_and_tab() {
    let mut state = AppState::pack_form();
    type_text(&mut state, "sr");
    let (_, source_cursor) = build_frame(&state);
    let source_position = match source_cursor {
        Some(value) => value,
        None => panic!("the form has no cursor"),
    };
    assert_eq!(usize::from(source_position.0), 21);
    press(&mut state, KeyCode::Backspace);
    let (_, shorter_cursor) = build_frame(&state);
    let shorter_position = match shorter_cursor {
        Some(value) => value,
        None => panic!("the form has no cursor"),
    };
    assert_eq!(shorter_position.0, source_position.0 - 1);
    press(&mut state, KeyCode::Tab);
    type_text(&mut state, "out");
    let (frame, target_cursor) = build_frame(&state);
    let target_position = match target_cursor {
        Some(value) => value,
        None => panic!("the form has no cursor"),
    };
    assert_eq!(target_position.1, source_position.1);
    let lines = frame_lines(&frame);
    let line = match lines.get(usize::from(target_position.1)) {
        Some(value) => *value,
        None => panic!("form line is missing"),
    };
    let visible = visible_text(line);
    let prefix = String::from("van pack --source [s] --target [out");
    assert!(visible.starts_with(&prefix));
    assert_eq!(usize::from(target_position.0), prefix.len());
}

#[test]
fn tab_moves_the_highlight_to_the_target_field() {
    let mut state = AppState::pack_form();
    type_text(&mut state, "src");
    let source_active = frame_of(&state);
    assert!(source_active.contains("\x1b[1;32msrc\x1b[0m"));
    assert!(!source_active.contains("\x1b[1;32mout\x1b[0m"));
    press(&mut state, KeyCode::Tab);
    type_text(&mut state, "out");
    let target_active = frame_of(&state);
    assert!(!target_active.contains("\x1b[1;32msrc\x1b[0m"));
    assert!(target_active.contains("\x1b[1;32mout\x1b[0m"));
}

#[test]
fn backspace_removes_the_last_typed_character() {
    let mut state = AppState::pack_form();
    type_text(&mut state, "abc");
    press(&mut state, KeyCode::Backspace);
    let frame = visible_frame_of(&state);
    assert!(
        frame.contains("van pack --source [ab] --target []"),
        "unexpected form line: {frame}"
    );
}

#[test]
fn enter_keeps_an_incomplete_form_open() {
    let mut state = AppState::pack_form();
    type_text(&mut state, "src");
    let finished = press(&mut state, KeyCode::Enter);
    assert!(finished.is_none(), "an incomplete form submitted");
    let frame = visible_frame_of(&state);
    assert!(frame.contains(&separator()), "the form closed");
}

#[test]
fn enter_returns_the_entered_paths() {
    let mut state = AppState::pack_form();
    type_text(&mut state, "src");
    press(&mut state, KeyCode::Tab);
    type_text(&mut state, "out");
    let finished = press(&mut state, KeyCode::Enter);
    let selection = match finished {
        Some(value) => value,
        None => panic!("a complete form did not submit"),
    };
    assert!(matches!(selection.choice(), MenuChoice::Pack));
    assert_eq!(selection.source(), Some("src"));
    assert_eq!(selection.target(), Some("out"));
}

#[test]
fn esc_closes_the_form_and_keeps_the_menu() {
    let mut state = AppState::pack_form();
    type_text(&mut state, "src");
    let finished = press(&mut state, KeyCode::Esc);
    assert!(finished.is_none(), "Esc left the application");
    let frame = visible_frame_of(&state);
    assert!(frame.contains("> Pack"), "selection was lost");
    assert!(!frame.contains(&separator()), "the form stayed open");
    assert!(!frame.contains("--source"), "the form line stayed");
    assert!(frame.contains(MENU_HINT), "the menu hint did not return");
}

#[test]
fn verify_form_asks_only_for_a_source() {
    let mut state = AppState::verify_form();
    type_text(&mut state, "arch");
    let frame = visible_frame_of(&state);
    assert!(frame.contains("van verify --source [arch]"));
    assert!(!frame.contains("--target"), "verify grew a target field");
    assert!(frame.contains(FORM_HINT));
    let finished = press(&mut state, KeyCode::Enter);
    let selection = match finished {
        Some(value) => value,
        None => panic!("a complete verify form did not submit"),
    };
    assert!(matches!(selection.choice(), MenuChoice::Verify));
    assert_eq!(selection.source(), Some("arch"));
    assert_eq!(selection.target(), None);
}

#[test]
fn verify_form_keeps_its_only_field_active() {
    let mut state = AppState::verify_form();
    type_text(&mut state, "a");
    press(&mut state, KeyCode::Tab);
    type_text(&mut state, "b");
    let frame = visible_frame_of(&state);
    assert!(frame.contains("van verify --source [ab]"));
}

#[test]
fn q_quits_from_the_menu() {
    let mut state = AppState::menu(0);
    let finished = press(&mut state, KeyCode::Char('q'));
    let selection = match finished {
        Some(value) => value,
        None => panic!("q did not quit"),
    };
    assert!(matches!(selection.choice(), MenuChoice::Quit));
    assert_eq!(selection.source(), None);
    assert_eq!(selection.target(), None);
}

#[test]
fn enter_on_the_quit_row_quits_without_a_form() {
    let mut state = AppState::menu(3);
    let finished = press(&mut state, KeyCode::Enter);
    let selection = match finished {
        Some(value) => value,
        None => panic!("the quit row did not quit"),
    };
    assert!(matches!(selection.choice(), MenuChoice::Quit));
}

#[test]
fn down_moves_the_marker_in_the_frame() {
    let mut state = AppState::menu(0);
    press(&mut state, KeyCode::Down);
    let frame = visible_frame_of(&state);
    assert!(frame.contains("> Verify"));
}
