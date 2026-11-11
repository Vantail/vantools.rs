//! Terminal primitives: a carriage-return-safe line writer, a raw-mode
//! session guard, and a minimal cross-platform input layer.

use std::io;
use std::io::Write;
use std::time::Duration;

use crossterm::cursor::{Hide, Show};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};

use crate::common::Error;

/// Writes lines with an explicit `\r\n` terminator.
///
/// In raw mode a bare `\n` moves the cursor down without returning it to
/// column 0, which makes consecutive lines drift diagonally. Every byte
/// written by this type therefore goes through [`LineWriter::emit`],
/// which normalizes each `\n` to `\r\n`, so a bare `\n` never reaches the
/// inner writer.
pub struct LineWriter<W: Write> {
    inner: W,
}

impl<W: Write> LineWriter<W> {
    /// Wrap the given writer.
    pub fn new(inner: W) -> Self {
        return LineWriter {
            inner,
        };
    }

    /// Write `text` with newlines normalized, followed by one `\r\n`.
    ///
    /// # Errors
    ///
    /// Returns the error reported by the inner writer.
    pub fn write_line(&mut self, text: &str) -> io::Result<()> {
        let emitted = self.emit(text);
        let written = match emitted {
            Ok(()) => self.inner.write_all(b"\r\n"),
            Err(error) => Err(error),
        };
        return written;
    }

    /// Write every line of `text` as its own terminated line.
    ///
    /// An empty input writes nothing.
    ///
    /// # Errors
    ///
    /// Returns the error reported by the inner writer.
    pub fn write_block(&mut self, text: &str) -> io::Result<()> {
        for line in text.lines() {
            let written = self.write_line(line);
            match written {
                Ok(()) => {}
                Err(error) => return Err(error),
            }
        }
        return Ok(());
    }

    /// Flush the inner writer.
    ///
    /// # Errors
    ///
    /// Returns the error reported by the inner writer.
    pub fn flush(&mut self) -> io::Result<()> {
        return self.inner.flush();
    }

    /// Consume the line writer and return the inner writer.
    pub fn into_inner(self) -> W {
        return self.inner;
    }

    /// Write `text` with every `\n` replaced by `\r\n`.
    ///
    /// This is the only place bytes reach the inner writer, so callers
    /// cannot bypass the normalization.
    fn emit(&mut self, text: &str) -> io::Result<()> {
        let mut normalized = String::with_capacity(text.len());
        for character in text.chars() {
            match character {
                '\n' => {
                    normalized.push('\r');
                    normalized.push('\n');
                }
                _ => {
                    normalized.push(character);
                }
            }
        }
        return self.inner.write_all(normalized.as_bytes());
    }
}

/// Guard for the terminal's raw mode and alternate screen.
///
/// The terminal stays in the interactive state until the guard is
/// dropped, and the guard restores the previous state when it is.
#[must_use = "the terminal stays in raw mode until the session is dropped"]
pub struct TerminalSession {
    // The guard owns no resources of its own: it exists to run the
    // restore sequence in `Drop` and to prevent construction from
    // outside this module.
    _guard: (),
}

impl TerminalSession {
    /// Enter raw mode and switch to the alternate screen.
    ///
    /// The terminal is cleared and the cursor is hidden, in that order,
    /// after raw mode has been enabled.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when no terminal is attached, which is
    /// the case for pipes and CI runs, or when the terminal refuses the
    /// requested mode. Any step that already succeeded is reversed before
    /// the error is returned.
    pub fn enter() -> Result<TerminalSession, Error> {
        let raw = crossterm::terminal::enable_raw_mode();
        match raw {
            Ok(()) => {}
            Err(error) => return Err(enter_failure(&error)),
        }
        let entered = crossterm::execute!(
            io::stdout(),
            EnterAlternateScreen,
            Clear(ClearType::All),
            Hide
        );
        match entered {
            Ok(()) => {
                return Ok(TerminalSession {
                    _guard: (),
                });
            }
            Err(error) => {
                restore();
                return Err(enter_failure(&error));
            }
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        restore();
    }
}

/// Restore the terminal state after an interactive session.
///
/// The cursor is shown first, then the alternate screen is left, and
/// finally raw mode is disabled. Running this sequence twice is
/// harmless: showing an already visible cursor, leaving an already left
/// alternate screen, and disabling inactive raw mode all succeed, which
/// is why the failure path of `enter` and `Drop` can both call it.
fn restore() {
    // Restore is best-effort because `Drop` cannot report failures, so
    // the result of every step is deliberately discarded. Running the
    // sequence twice is harmless: the cursor can be shown again, the
    // alternate screen can be left again, and raw mode can be disabled
    // again.
    let _ = crossterm::execute!(io::stdout(), Show);
    let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
    let _ = crossterm::terminal::disable_raw_mode();
}

/// Build the error reported when the terminal cannot be configured.
fn enter_failure(error: &io::Error) -> Error {
    return Error::Runtime(format!("failed to enter terminal mode: {error}"));
}

/// A terminal event the interactive interface reacts to.
///
/// The enum is `non_exhaustive` because `clippy::exhaustive_enums` is
/// denied for this crate.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEvent {
    /// The user asked to leave the interactive interface.
    Quit,
    /// A key press forwarded to the caller.
    Key {
        /// The pressed key.
        code: KeyCode,
        /// The modifiers held while the key was pressed.
        modifiers: KeyModifiers,
    },
    /// The terminal window changed size.
    Resize {
        /// New width in columns.
        width: u16,
        /// New height in rows.
        height: u16,
    },
}

/// Map a raw crossterm event to the interactive layer's vocabulary.
///
/// Returns `None` for events the interface ignores.
pub fn classify(event: Event) -> Option<InputEvent> {
    return match event {
        Event::Key(key) => classify_key(key),
        Event::Resize(width, height) => Some(InputEvent::Resize {
            width,
            height,
        }),
        // `crossterm::event::Event` is `#[non_exhaustive]`, so this
        // wildcard arm is mandatory; it also covers mouse, focus, and
        // paste events.
        _ => None,
    };
}

/// Classify a key event, ignoring key releases.
fn classify_key(key: KeyEvent) -> Option<InputEvent> {
    // Windows delivers a `Press` and a `Release` event for every key.
    // Handling both would double every keystroke, so only presses count.
    if key.kind != KeyEventKind::Press {
        return None;
    }
    let quit = match key.code {
        KeyCode::Esc => true,
        KeyCode::Char('c') => key.modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    };
    if quit {
        return Some(InputEvent::Quit);
    }
    return Some(InputEvent::Key {
        code: key.code,
        modifiers: key.modifiers,
    });
}

/// Wait up to `timeout` for the next input event.
///
/// Returns `Ok(None)` when the timeout elapses without an event, and
/// when the received event is one the interface ignores.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when polling or reading the terminal fails,
/// for example when no terminal is attached.
pub fn poll_input(timeout: Duration) -> Result<Option<InputEvent>, Error> {
    let ready = crossterm::event::poll(timeout);
    let available = match ready {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to poll terminal input: {error}"
            )));
        }
    };
    if !available {
        return Ok(None);
    }
    let event = crossterm::event::read();
    let received = match event {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to read terminal input: {error}"
            )));
        }
    };
    return Ok(classify(received));
}

#[cfg(test)]
mod tests {
    use crossterm::event::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    };

    use super::{InputEvent, classify};

    #[test]
    fn escape_press_classifies_as_quit() {
        let event = Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(classify(event), Some(InputEvent::Quit));
    }

    #[test]
    fn control_c_classifies_as_quit() {
        let event = Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(classify(event), Some(InputEvent::Quit));
    }

    #[test]
    fn plain_c_classifies_as_key() {
        let event =
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        let expected = InputEvent::Key {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(classify(event), Some(expected));
    }

    #[test]
    fn arrow_key_classifies_as_key() {
        let event = Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        let expected = InputEvent::Key {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(classify(event), Some(expected));
    }

    #[test]
    fn escape_release_classifies_as_none() {
        let event = Event::Key(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ));
        assert_eq!(classify(event), None);
    }

    #[test]
    fn resize_classifies_as_resize() {
        let event = Event::Resize(80, 24);
        let expected = InputEvent::Resize {
            width: 80,
            height: 24,
        };
        assert_eq!(classify(event), Some(expected));
    }

    #[test]
    fn focus_gained_classifies_as_none() {
        assert_eq!(classify(Event::FocusGained), None);
    }
}
