//! Integration tests for the menu widget.
//!
//! Every test drives the widget through its public API only: the menu
//! is built with `Menu::new`, moved with `move_up` and `move_down`, fed
//! with constructed `InputEvent` values, and rendered into an in-memory
//! `LineWriter`.

use crossterm::event::{KeyCode, KeyModifiers};
use vantools::common::Error;
use vantools::tui::menu::{Menu, MenuOutcome};
use vantools::tui::terminal::{InputEvent, LineWriter};

/// Title used by the tests that only care about the options.
const TITLE: &str = "Title";

/// Build the three options most tests select between.
fn sample_options() -> Vec<String> {
    let options: Vec<String> = vec![
        String::from("Alpha"),
        String::from("Beta"),
        String::from("Gamma"),
    ];
    return options;
}

/// Build the three-option menu most tests start from.
fn sample_menu() -> Menu {
    let options = sample_options();
    let built = Menu::new(TITLE, &options);
    return match built {
        Ok(menu) => menu,
        Err(error) => panic!("the sample menu should build: {error}"),
    };
}

/// Build a one-option menu.
fn one_option_menu() -> Menu {
    let options = vec![String::from("Only")];
    let built = Menu::new(TITLE, &options);
    return match built {
        Ok(menu) => menu,
        Err(error) => panic!("the one-option menu should build: {error}"),
    };
}

/// Return the error message of a rejected menu, or `None` when the menu
/// was accepted.
fn rejection_message(title: &str, options: &[String]) -> Option<String> {
    let built = Menu::new(title, options);
    return match built {
        Ok(_) => None,
        Err(Error::Runtime(message)) => Some(message),
        Err(_) => panic!("the rejection should be a runtime error"),
    };
}

#[test]
fn new_builds_menu_with_three_options() {
    let menu = sample_menu();
    assert_eq!(menu.selected_index(), 0);
}

#[test]
fn new_rejects_an_empty_option_list() {
    let options: Vec<String> = Vec::new();
    let message = rejection_message(TITLE, &options);
    let expected = Some(String::from("menu requires at least one option"));
    assert_eq!(message, expected);
}

#[test]
fn new_rejects_an_option_with_a_line_break() {
    let options = vec![String::from("Al\npha")];
    let message = rejection_message(TITLE, &options);
    let expected = Some(String::from("menu text must not contain line breaks"));
    assert_eq!(message, expected);
}

#[test]
fn new_rejects_a_title_with_a_line_break() {
    let options = sample_options();
    let message = rejection_message("Ti\rrtle", &options);
    let expected = Some(String::from("menu text must not contain line breaks"));
    assert_eq!(message, expected);
}

#[test]
fn move_up_from_the_first_option_wraps_to_the_last() {
    let mut menu = sample_menu();
    menu.move_up();
    assert_eq!(menu.selected_index(), 2);
}

#[test]
fn move_down_from_the_last_option_wraps_to_the_first() {
    let mut menu = sample_menu();
    menu.move_up();
    menu.move_down();
    assert_eq!(menu.selected_index(), 0);
}

#[test]
fn move_down_from_the_first_option_selects_the_second() {
    let mut menu = sample_menu();
    menu.move_down();
    assert_eq!(menu.selected_index(), 1);
}

#[test]
fn a_one_option_menu_stays_on_the_first_option() {
    let mut menu = one_option_menu();
    menu.move_up();
    assert_eq!(menu.selected_index(), 0);
    menu.move_down();
    assert_eq!(menu.selected_index(), 0);
}

#[test]
fn render_writes_the_title_and_every_option() {
    let menu = sample_menu();
    let mut writer = LineWriter::new(Vec::new());
    let written = menu.render(&mut writer);
    assert!(written.is_ok());
    let actual = writer.into_inner();
    let expected =
        String::from("Title\r\n> Alpha\r\n  Beta\r\n  Gamma\r\n").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn render_marks_the_second_option_after_move_down() {
    let mut menu = sample_menu();
    menu.move_down();
    let mut writer = LineWriter::new(Vec::new());
    let written = menu.render(&mut writer);
    assert!(written.is_ok());
    let actual = writer.into_inner();
    let expected =
        String::from("Title\r\n  Alpha\r\n> Beta\r\n  Gamma\r\n").into_bytes();
    assert_eq!(actual, expected);
}

#[test]
fn handle_quit_returns_quit() {
    let mut menu = sample_menu();
    let outcome = menu.handle(InputEvent::Quit);
    assert_eq!(outcome, MenuOutcome::Quit);
    assert_eq!(menu.selected_index(), 0);
}

#[test]
fn handle_up_returns_redraw_and_wraps() {
    let mut menu = sample_menu();
    let event = InputEvent::Key {
        code: KeyCode::Up,
        modifiers: KeyModifiers::NONE,
    };
    let outcome = menu.handle(event);
    assert_eq!(outcome, MenuOutcome::Redraw);
    assert_eq!(menu.selected_index(), 2);
}

#[test]
fn handle_enter_returns_the_selected_index() {
    let mut menu = sample_menu();
    menu.move_down();
    let event = InputEvent::Key {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
    };
    let expected = MenuOutcome::Selected {
        index: 1,
    };
    let first = menu.handle(event);
    assert_eq!(first, expected);
    let second = menu.handle(event);
    assert_eq!(second, expected);
    assert_eq!(menu.selected_index(), 1);
}

#[test]
fn handle_ignores_a_resize_and_other_keys() {
    let mut menu = sample_menu();
    let resized = menu.handle(InputEvent::Resize {
        width: 80,
        height: 24,
    });
    assert_eq!(resized, MenuOutcome::Nothing);
    let other = menu.handle(InputEvent::Key {
        code: KeyCode::Char('x'),
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(other, MenuOutcome::Nothing);
    assert_eq!(menu.selected_index(), 0);
}
