//! Menu widget: a titled list of options with one selected entry.
//!
//! The widget is a pure component. It holds the selection state and
//! writes one frame through a [`LineWriter`], and it never enters raw
//! mode, moves the cursor, clears the screen, or reads input. Handling
//! an input event only reports what the caller should do next, so the
//! drawing loop stays in the caller.

use std::io;
use std::io::Write;

use crossterm::event::KeyCode;

use crate::common::Error;
use crate::tui::terminal::{InputEvent, LineWriter};

/// Prefix drawn before the selected option.
const SELECTED_PREFIX: &str = "> ";

/// Prefix drawn before every other option.
const UNSELECTED_PREFIX: &str = "  ";

/// The result of handling one input event.
///
/// The enum is `non_exhaustive` because `clippy::exhaustive_enums` is
/// denied for this crate.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuOutcome {
    /// The event changed nothing the widget draws.
    Nothing,
    /// The selection moved and the frame must be drawn again.
    Redraw,
    /// The user confirmed the selected option.
    Selected {
        /// Index of the confirmed option.
        index: usize,
    },
    /// The user asked to leave the interactive interface.
    Quit,
}

/// A titled list of options with one selected entry.
pub struct Menu {
    /// Heading drawn above the options.
    title: String,
    /// The selectable options, in display order.
    options: Vec<String>,
    /// Index of the selected option in the option list.
    selected: usize,
}

impl Menu {
    /// Build a menu from a title and the options it offers.
    ///
    /// The first option starts selected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when `options` is empty, and when the
    /// title or any option contains a line break.
    pub fn new(title: &str, options: &[String]) -> Result<Menu, Error> {
        // Every frame line goes through `LineWriter`, which terminates
        // it with `\r\n`. Text that already carries a line break would
        // add a line of its own, and a bare `\n` moves the cursor down
        // without returning it to column 0. Rejecting breaks keeps the
        // frame at exactly one line per item and prevents cascaded
        // (staircase) output.
        let mut broken = has_line_break(title);
        for option in options {
            if has_line_break(option) {
                broken = true;
            }
        }
        let rejection = match options.is_empty() {
            true => Some(String::from("menu requires at least one option")),
            false => match broken {
                true => {
                    Some(String::from("menu text must not contain line breaks"))
                }
                false => None,
            },
        };
        return match rejection {
            Some(message) => Err(Error::Runtime(message)),
            None => {
                let mut copied: Vec<String> = Vec::new();
                for option in options {
                    copied.push(String::from(option.as_str()));
                }
                Ok(Menu {
                    title: String::from(title),
                    options: copied,
                    selected: 0,
                })
            }
        };
    }

    /// Return the index of the selected option.
    pub fn selected_index(&self) -> usize {
        return self.selected;
    }

    /// Move the selection up by one option.
    ///
    /// The move wraps around: a press on the first option selects the
    /// last one. A menu always holds at least one option, so the last
    /// index never underflows, and a one-option menu keeps index `0`.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            return;
        }
        self.selected = self.options.len() - 1;
    }

    /// Move the selection down by one option.
    ///
    /// The move wraps around: a press on the last option selects the
    /// first one, and a one-option menu keeps index `0`.
    pub fn move_down(&mut self) {
        let last = self.options.len() - 1;
        if self.selected < last {
            self.selected += 1;
            return;
        }
        self.selected = 0;
    }

    /// Apply one input event and report what the caller should do next.
    ///
    /// Only `Up`, `Down`, and `Enter` change the outcome; every other
    /// key and a resize leave the selection as it is. Modifiers are
    /// deliberately ignored for the navigation keys, because the widget
    /// has no modifier commands, so `Up` with Shift or Control still
    /// moves the selection.
    pub fn handle(&mut self, event: InputEvent) -> MenuOutcome {
        return match event {
            InputEvent::Quit => MenuOutcome::Quit,
            InputEvent::Key {
                code: KeyCode::Up,
                modifiers: _,
            } => {
                self.move_up();
                MenuOutcome::Redraw
            }
            InputEvent::Key {
                code: KeyCode::Down,
                modifiers: _,
            } => {
                self.move_down();
                MenuOutcome::Redraw
            }
            InputEvent::Key {
                code: KeyCode::Enter,
                modifiers: _,
            } => MenuOutcome::Selected {
                index: self.selected_index(),
            },
            // `crossterm::event::KeyCode` is `#[non_exhaustive]`, so this
            // last arm is mandatory. It also covers every key the widget
            // does not act on, and a resize changes nothing the widget
            // draws.
            InputEvent::Resize {
                ..
            }
            | InputEvent::Key {
                code: _,
                modifiers: _,
            } => MenuOutcome::Nothing,
        };
    }

    /// Write one frame of the menu through `writer`.
    ///
    /// The frame is the title line followed by one line per option, in
    /// order, so it has exactly `1 + options.len()` lines. The selected
    /// option is drawn as `> {label}` and every other option as
    /// `  {label}`, and the label follows the prefix directly. A frame
    /// of a three-option menu therefore reads:
    ///
    /// ```text
    /// Title
    /// > Alpha
    ///   Beta
    /// ```
    ///
    /// No blank lines, trailing separators, or ANSI sequences are
    /// written, and [`LineWriter`] guarantees every line ends with
    /// `\r\n`, so `render` never emits a bare `\n`.
    ///
    /// # Errors
    ///
    /// Returns the error reported by the underlying writer.
    pub fn render<W: Write>(
        &self,
        writer: &mut LineWriter<W>,
    ) -> io::Result<()> {
        let title_written = writer.write_line(&self.title);
        match title_written {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        for (index, option) in self.options.iter().enumerate() {
            let prefix = match index == self.selected {
                true => SELECTED_PREFIX,
                false => UNSELECTED_PREFIX,
            };
            let line = format!("{prefix}{option}");
            let written = writer.write_line(&line);
            match written {
                Ok(()) => {}
                Err(error) => return Err(error),
            }
        }
        return Ok(());
    }
}

/// Returns true when `text` contains a line break.
fn has_line_break(text: &str) -> bool {
    return text.contains('\n') || text.contains('\r');
}
