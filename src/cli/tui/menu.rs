//! Interactive start menu for the `van` binary.
//!
//! The menu is shown when `van` runs without arguments. It draws the
//! wordmark, the slogan, and the four top-level commands, and returns
//! the command the user confirmed. Raw mode and the alternate screen
//! belong to a [`TerminalGuard`], which restores both when the menu
//! leaves its scope, including when it returns an error.
//!
//! The layout is written for at least 80 columns. A narrower window
//! gets a short notice and exits on the next key press, because a
//! wrapped frame breaks the wordmark and scatters the menu rows.

use std::io;
use std::io::IsTerminal;
use std::io::Write;

use crossterm::cursor::MoveTo;
use crossterm::event;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::queue;
use crossterm::style::{Print, Stylize};
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
    disable_raw_mode, enable_raw_mode, size,
};
use vantools::common::Error;

/// The start menu wordmark, one entry per row.
///
/// The rows are kept verbatim: the face depends on the exact column of
/// every character, so no row may be reflowed or trimmed.
const LOGO_ROWS: [&str; 5] = [
    " __     __     _       _   _  ",
    " \\ \\   / /    / \\     | \\ | | ",
    "  \\ \\ / /    / _ \\    |  \\| | ",
    "   \\ V /    / ___ \\   | |\\  | ",
    "    \\_/    /_/   \\_\\  |_| \\_| ",
];

/// Slogan shown below the wordmark.
const SLOGAN: &str = "The van Plugin- and Packagemanager CLI.";

/// Hint line shown below the commands.
const HINT: &str = "  ↑↓ Select | Enter Confirm | Q Quit";

/// Minimum number of columns the start menu is laid out for.
const MIN_WIDTH: u16 = 80;

/// Message shown when the window is narrower than 80 columns.
const NARROW_MESSAGE: &str =
    "Terminal is too narrow. Please resize your window to at least 80 columns.";

/// Prompt shown below the narrow message.
const NARROW_HINT: &str = "Press any key to exit.";

/// Width the command names are padded to before the description.
const LABEL_WIDTH: usize = 10;

/// Command the user picked in the start menu.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum MenuChoice {
    /// Compress a plugin directory into an archive.
    Pack,
    /// Validate an archive before it is used.
    Verify,
    /// Extract an archive into a plugin directory.
    Unpack,
    /// Leave the application.
    Quit,
}

/// One selectable row of the start menu.
struct MenuEntry {
    /// Command returned when the row is confirmed.
    choice: MenuChoice,
    /// Command name shown in the row.
    label: &'static str,
    /// One-line description shown after the name.
    description: &'static str,
}

/// Rows of the start menu, in display order.
const MENU_ENTRIES: [MenuEntry; 4] = [
    MenuEntry {
        choice: MenuChoice::Pack,
        label: "Pack",
        description: "Compress a plugin directory",
    },
    MenuEntry {
        choice: MenuChoice::Verify,
        label: "Verify",
        description: "Validate an archive",
    },
    MenuEntry {
        choice: MenuChoice::Unpack,
        label: "Unpack",
        description: "Extract an archive",
    },
    MenuEntry {
        choice: MenuChoice::Quit,
        label: "Quit",
        description: "Exit the application",
    },
];

/// Show the start menu and return the confirmed command.
///
/// The terminal must be interactive: raw mode cannot be enabled for a
/// pipe, and a redirected stdin would never deliver a key press.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when there is no interactive terminal,
/// when raw mode or the alternate screen cannot be enabled, or when
/// the terminal cannot be drawn or read.
pub fn show_menu() -> Result<MenuChoice, Error> {
    if !is_interactive() {
        return Err(Error::Runtime(String::from(
            "the start menu requires an interactive terminal",
        )));
    }
    let _guard = TerminalGuard::acquire()?;
    if terminal_width() < MIN_WIDTH {
        return show_narrow_notice();
    }
    let mut selected: usize = 0;
    loop {
        draw_menu(selected)?;
        let event = match event::read() {
            Ok(value) => value,
            Err(error) => {
                let message = format!("failed to read a key press: {error}");
                return Err(Error::Runtime(message));
            }
        };
        let key = match event {
            Event::Key(key) => key,
            _ => continue,
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        selected = match key.code {
            KeyCode::Up => previous_index(selected),
            KeyCode::Down => next_index(selected),
            KeyCode::Enter => return Ok(choice_at(selected)),
            KeyCode::Char('q') => return Ok(MenuChoice::Quit),
            KeyCode::Esc => return Ok(MenuChoice::Quit),
            _ => selected,
        };
    }
}

/// Clear the terminal once the menu has closed.
///
/// The guard has already restored the terminal when this runs, so the
/// help text of the chosen command starts on an empty screen. The clear
/// is best effort: the command's output matters more than the screen.
pub fn clear_screen() {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, Clear(ClearType::All), MoveTo(0, 0));
    return;
}

/// Return the current terminal width in columns.
///
/// A measurement that the platform cannot provide falls back to the
/// minimum width, so the menu still draws instead of failing.
fn terminal_width() -> u16 {
    let measured = size();
    return match measured {
        Ok(value) => value.0,
        Err(_) => MIN_WIDTH,
    };
}

/// Explain that the window is too narrow and wait for one key press.
fn show_narrow_notice() -> Result<MenuChoice, Error> {
    let mut text = String::from(NARROW_MESSAGE);
    text.push_str("\n\n");
    text.push_str(NARROW_HINT);
    text.push('\n');
    draw_text(&text)?;
    let read = event::read();
    return match read {
        Ok(_) => Ok(MenuChoice::Quit),
        Err(error) => {
            let message = format!("failed to read a key press: {error}");
            Err(Error::Runtime(message))
        }
    };
}

/// Report whether the menu can read keys and draw at all.
fn is_interactive() -> bool {
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        return false;
    }
    let stdout = io::stdout();
    return stdout.is_terminal();
}

/// Owns raw mode and the alternate screen while the menu runs.
///
/// The guard is created before the alternate screen is entered, so a
/// failure at any later point still restores the terminal on drop.
struct TerminalGuard;

impl TerminalGuard {
    /// Enter the alternate screen and switch the terminal to raw mode.
    fn acquire() -> Result<Self, Error> {
        if let Err(error) = enable_raw_mode() {
            let message = format!("failed to enable raw mode: {error}");
            return Err(Error::Runtime(message));
        }
        let guard = TerminalGuard;
        let mut stdout = io::stdout();
        let entered = execute!(stdout, EnterAlternateScreen);
        return match entered {
            Ok(()) => Ok(guard),
            Err(error) => {
                let message =
                    format!("failed to enter the alternate screen: {error}");
                Err(Error::Runtime(message))
            }
        };
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // The terminal is restored best effort: a stream that fails here
        // cannot report its own failure either.
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen);
        return;
    }
}

/// Render the whole menu for the given selection.
fn build_frame(selected: usize) -> String {
    let mut frame = String::new();
    for row in LOGO_ROWS {
        frame.push_str(row);
        frame.push('\n');
    }
    frame.push('\n');
    frame.push_str(SLOGAN);
    frame.push_str("\n\n");
    let mut index: usize = 0;
    while index < MENU_ENTRIES.len() {
        frame.push_str(&render_entry(index, selected));
        frame.push('\n');
        index += 1;
    }
    frame.push('\n');
    frame.push_str(HINT);
    frame.push('\n');
    return frame;
}

/// Render one row, marking and highlighting it when it is selected.
fn render_entry(index: usize, selected: usize) -> String {
    let entry = match MENU_ENTRIES.get(index) {
        Some(value) => value,
        None => return String::new(),
    };
    let picked = index == selected;
    let indicator = match picked {
        true => "> ",
        false => "  ",
    };
    let cell = pad_label(entry.label);
    let mut line = match picked {
        // Only the selected row carries attributes.
        true => format!("{}", format!("{indicator}{cell}").bold().magenta()),
        false => format!("{indicator}{cell}"),
    };
    line.push_str(entry.description);
    return line;
}

/// Pad a command name to the column its description starts in.
fn pad_label(label: &str) -> String {
    let mut cell = String::from(label);
    let mut width: usize = label.len();
    while width < LABEL_WIDTH {
        cell.push(' ');
        width += 1;
    }
    return cell;
}

/// Clear the screen and draw the menu with the current selection.
fn draw_menu(selected: usize) -> Result<(), Error> {
    let frame = build_frame(selected);
    return draw_text(&frame);
}

/// Replace the screen with `text`, starting at the top-left corner.
///
/// Clearing first keeps a shorter frame from leaving parts of the
/// previous selection behind.
fn draw_text(text: &str) -> Result<(), Error> {
    let mut stdout = io::stdout();
    let queued =
        queue!(stdout, Clear(ClearType::All), MoveTo(0, 0), Print(text));
    if let Err(error) = queued {
        let message = format!("failed to draw the menu: {error}");
        return Err(Error::Runtime(message));
    }
    let flushed = stdout.flush();
    return match flushed {
        Ok(()) => Ok(()),
        Err(error) => {
            let message = format!("failed to flush the menu: {error}");
            Err(Error::Runtime(message))
        }
    };
}

/// Return the command of the row at `index`.
///
/// The index always comes from the selection, so a miss can only happen
/// if the entries and the index disagree; quitting is the safe answer.
fn choice_at(index: usize) -> MenuChoice {
    let entry = MENU_ENTRIES.get(index);
    return match entry {
        Some(value) => value.choice,
        None => MenuChoice::Quit,
    };
}

/// Return the index above `current`, wrapping to the last row.
fn previous_index(current: usize) -> usize {
    if current == 0 {
        return MENU_ENTRIES.len() - 1;
    }
    return current - 1;
}

/// Return the index below `current`, wrapping to the first row.
fn next_index(current: usize) -> usize {
    let next = current + 1;
    if next >= MENU_ENTRIES.len() {
        return 0;
    }
    return next;
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
