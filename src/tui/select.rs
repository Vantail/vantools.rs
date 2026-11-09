//! Generic interactive selection menu for the TUI.
//!
//! [`show_menu`] renders any list of options and returns the chosen
//! index. It knows nothing about the `van` subcommands, so future
//! menus such as a file picker can reuse it unchanged.

use std::io;
use std::io::Write;

use crossterm::cursor::MoveTo;
use crossterm::event;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::execute;
use crossterm::style::Attribute;
use crossterm::style::Color;
use crossterm::style::Print;
use crossterm::style::ResetColor;
use crossterm::style::SetAttribute;
use crossterm::style::SetForegroundColor;
use crossterm::terminal::Clear;
use crossterm::terminal::ClearType;

use super::terminal::TerminalGuard;
use crate::common::Error;

/// Show an interactive selection menu, return the chosen index.
///
/// `Up`/`Down` or `k`/`j` move the highlight with wraparound,
/// `Enter` selects, and `q` or `Esc` quits with [`None`]. An empty
/// option list selects nothing and draws nothing.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the terminal cannot be prepared
/// or a key event cannot be read.
pub fn show_menu(
    title: &str,
    options: &[&str],
) -> Result<Option<usize>, Error> {
    if options.is_empty() {
        return Ok(None);
    }
    let _guard = TerminalGuard::enter()?;
    let last: usize = options.len() - 1;
    let mut selected: usize = 0;
    loop {
        draw_menu(title, options, selected);
        let event = match event::read() {
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read key event: {error}"
                )));
            }
        };
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if selected == 0 {
                        selected = last;
                    } else {
                        selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if selected == last {
                        selected = 0;
                    } else {
                        selected += 1;
                    }
                }
                KeyCode::Enter => return Ok(Some(selected)),
                KeyCode::Esc | KeyCode::Char('q') => {
                    return Ok(None);
                }
                _ => {}
            }
        }
    }
}

/// Draw the title and every option, highlighting the selected row.
///
/// The screen is cleared first so a shorter redraw cannot leave stale
/// rows behind. The selected row carries a bold magenta `>` marker
/// and the others a plain two-space indent. Drawing is best effort:
/// a closed stdout cannot report its own failure.
fn draw_menu(title: &str, options: &[&str], selected: usize) {
    let mut handle = io::stdout();
    let _ = execute!(handle, Clear(ClearType::All), MoveTo(0, 0));
    if !title.is_empty() {
        let _ = writeln!(handle, "{title}\n");
    }
    for (position, label) in options.iter().enumerate() {
        if position == selected {
            let _ = execute!(
                handle,
                SetAttribute(Attribute::Bold),
                SetForegroundColor(Color::Magenta),
                Print(format!("> {label}")),
                ResetColor,
                SetAttribute(Attribute::Reset),
                Print("\n"),
            );
        } else {
            let _ = writeln!(handle, "  {label}");
        }
    }
    return;
}
