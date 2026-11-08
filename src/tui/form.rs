//! Interactive parameter form for the `pack` command.
//!
//! When the start menu chooses `pack`, the form replaces the help text
//! with one editable line that mirrors the command line: the user types
//! a source and a target, moves between them with Tab, and submits with
//! Enter. Esc cancels. Raw mode and the alternate screen belong to a
//! guard, which restores both when the form leaves its scope, including
//! when it returns an error.

use std::io;
use std::io::IsTerminal;
use std::io::Write;

use crate::common::Error;
use crossterm::cursor::{MoveTo, Show};
use crossterm::event;
use crossterm::event::KeyModifiers;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
    disable_raw_mode, enable_raw_mode,
};

/// Text shown before the source value.
const SOURCE_PREFIX: &str = "van pack --source [";

/// Text between the source value and the target value.
const FIELD_SEPARATOR: &str = "] --target [";

/// Text after the target value.
const LINE_SUFFIX: &str = "]";

/// ANSI SGR sequence that marks the active field.
const ACTIVE_FIELD: &str = "\x1b[1;32m";

/// ANSI SGR sequence that resets every attribute.
const RESET: &str = "\x1b[0m";

/// Buffer index of the source field.
const SOURCE_FIELD: usize = 0;

/// Buffer index of the target field.
const TARGET_FIELD: usize = 1;

/// Buffers and active field of the `pack` parameter form.
#[derive(Default)]
pub struct FormState {
    /// Index of the field that receives typed characters.
    active_field: usize,
    /// Characters typed into the source field.
    source: String,
    /// Characters typed into the target field.
    target: String,
}

impl FormState {
    /// Create a form with both buffers empty and the source active.
    pub fn new() -> Self {
        return FormState::default();
    }

    /// Return the index of the active field.
    pub fn active_field(&self) -> usize {
        return self.active_field;
    }

    /// Return the characters typed into the source field.
    pub fn source(&self) -> &str {
        return &self.source;
    }

    /// Return the characters typed into the target field.
    pub fn target(&self) -> &str {
        return &self.target;
    }

    /// Switch to the other field.
    pub fn toggle_field(&mut self) {
        if self.active_field == SOURCE_FIELD {
            self.active_field = TARGET_FIELD;
            return;
        }
        self.active_field = SOURCE_FIELD;
        return;
    }

    /// Append `character` to the active field.
    pub fn push_char(&mut self, character: char) {
        if self.active_field == TARGET_FIELD {
            self.target.push(character);
            return;
        }
        self.source.push(character);
        return;
    }

    /// Remove the last character of the active field, if any.
    pub fn pop_char(&mut self) {
        if self.active_field == TARGET_FIELD {
            let _removed = self.target.pop();
            return;
        }
        let _removed = self.source.pop();
        return;
    }

    /// Return the entered values when both fields are non-empty.
    pub fn submit(&self) -> Option<(String, String)> {
        if self.source.is_empty() || self.target.is_empty() {
            return None;
        }
        return Some((self.source.clone(), self.target.clone()));
    }
}

/// What a key press means to the form loop.
enum FormOutcome {
    /// Keep the form open.
    Continue,
    /// Submit the buffers.
    Submit,
    /// Close the form without values.
    Cancel,
}

/// Show the `pack` form and return the entered values.
///
/// Returns `Some((source, target))` when Enter is pressed with both
/// fields filled in, and `None` when Esc or Ctrl+C cancels. Enter on an
/// incomplete form is ignored, so the user stays in the form instead of
/// submitting an unusable command line.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when there is no interactive terminal,
/// when raw mode or the alternate screen cannot be enabled, or when the
/// form cannot be drawn or read.
pub fn show_pack_form() -> Result<Option<(String, String)>, Error> {
    if !is_interactive() {
        return Err(Error::Runtime(String::from(
            "the pack form requires an interactive terminal",
        )));
    }
    let _guard = FormGuard::acquire()?;
    let mut state = FormState::new();
    loop {
        draw_form(&state)?;
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
        let outcome = handle_key(&mut state, &key);
        match outcome {
            FormOutcome::Continue => continue,
            FormOutcome::Cancel => return Ok(None),
            FormOutcome::Submit => {
                let values = state.submit();
                if values.is_some() {
                    return Ok(values);
                }
            }
        }
    }
}

/// Report whether the form can read keys and draw at all.
fn is_interactive() -> bool {
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        return false;
    }
    let stdout = io::stdout();
    return stdout.is_terminal();
}

/// Apply one key press to `state`.
fn handle_key(state: &mut FormState, key: &KeyEvent) -> FormOutcome {
    return match key.code {
        KeyCode::Tab => {
            state.toggle_field();
            FormOutcome::Continue
        }
        KeyCode::Esc => FormOutcome::Cancel,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            FormOutcome::Cancel
        }
        KeyCode::Enter => FormOutcome::Submit,
        KeyCode::Backspace => {
            state.pop_char();
            FormOutcome::Continue
        }
        KeyCode::Char(character) => {
            if character.is_control() {
                return FormOutcome::Continue;
            }
            state.push_char(character);
            FormOutcome::Continue
        }
        _ => FormOutcome::Continue,
    };
}

/// Render the form line with the active field highlighted.
pub fn build_frame(state: &FormState) -> String {
    let source = field_cell(state, SOURCE_FIELD, state.source());
    let target = field_cell(state, TARGET_FIELD, state.target());
    return format!(
        "{SOURCE_PREFIX}{source}{FIELD_SEPARATOR}{target}{LINE_SUFFIX}"
    );
}

/// Render one buffer, highlighted when it belongs to the active field.
fn field_cell(state: &FormState, index: usize, buffer: &str) -> String {
    if state.active_field() == index {
        return format!("{ACTIVE_FIELD}{buffer}{RESET}");
    }
    return String::from(buffer);
}

/// Return how many characters `text` holds.
///
/// Iterator adapters are not used in this crate, so the width is counted
/// with an explicit loop instead of `chars().count()`.
fn char_width(text: &str) -> usize {
    let mut width: usize = 0;
    for _character in text.chars() {
        width += 1;
    }
    return width;
}

/// Return the zero-based column the cursor occupies in the form line.
///
/// The column counts characters, not bytes, so a buffer with multi-byte
/// characters still puts the cursor after the text it holds.
pub fn cursor_column(state: &FormState) -> u16 {
    let mut column = SOURCE_PREFIX.len() + char_width(state.source());
    if state.active_field() == TARGET_FIELD {
        column += FIELD_SEPARATOR.len() + char_width(state.target());
    }
    let limit = usize::from(u16::MAX);
    if column > limit {
        return u16::MAX;
    }
    return column as u16;
}

/// Replace the screen with the current form line.
///
/// The cursor stays visible and is moved into the active field, so the
/// terminal's own cursor shows where typing lands.
fn draw_form(state: &FormState) -> Result<(), Error> {
    let line = build_frame(state);
    let column = cursor_column(state);
    let mut stdout = io::stdout();
    let queued = queue!(
        stdout,
        Clear(ClearType::All),
        MoveTo(0, 0),
        Print(line),
        Show,
        MoveTo(column, 0)
    );
    if let Err(error) = queued {
        let message = format!("failed to draw the form: {error}");
        return Err(Error::Runtime(message));
    }
    let flushed = stdout.flush();
    return match flushed {
        Ok(()) => Ok(()),
        Err(error) => {
            let message = format!("failed to flush the form: {error}");
            Err(Error::Runtime(message))
        }
    };
}

/// Owns raw mode and the alternate screen while the form runs.
///
/// The guard is created before the alternate screen is entered, so a
/// failure at any later point still restores the terminal on drop.
struct FormGuard;

impl FormGuard {
    /// Enter the alternate screen and switch the terminal to raw mode.
    fn acquire() -> Result<Self, Error> {
        if let Err(error) = enable_raw_mode() {
            let message = format!("failed to enable raw mode: {error}");
            return Err(Error::Runtime(message));
        }
        let guard = FormGuard;
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

impl Drop for FormGuard {
    fn drop(&mut self) {
        // The terminal is restored best effort: a stream that fails here
        // cannot report its own failure either.
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
        return;
    }
}
