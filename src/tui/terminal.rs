//! Safe terminal state handling for interactive TUI views.
//!
//! [`TerminalGuard`] switches the terminal to raw mode and the
//! alternate screen, then restores the previous state when it is
//! dropped. Holding the guard for the whole interactive view keeps a
//! panic or an early return from leaving the user's shell unusable.

use std::io;

use crossterm::cursor::Hide;
use crossterm::cursor::Show;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;

use crate::common::Error;

/// Guard restoring the terminal when an interactive view ends.
///
/// The guard owns no data; entering raw mode and the alternate screen
/// is the resource, and [`Drop`] releases it. The struct is
/// `non_exhaustive` so later phases can carry state without a
/// breaking change, which `clippy::exhaustive_structs` requires.
#[non_exhaustive]
pub struct TerminalGuard {
    /// Marker field; the terminal state itself is the guarded resource.
    _marker: (),
}

impl TerminalGuard {
    /// Enter raw mode, switch to the alternate screen, hide cursor.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when raw mode or the alternate
    /// screen cannot be enabled.
    pub fn enter() -> Result<Self, Error> {
        if let Err(error) = enable_raw_mode() {
            return Err(Error::Runtime(format!(
                "failed to enable raw mode: {error}"
            )));
        }
        let entered = execute!(io::stdout(), EnterAlternateScreen, Hide);
        return match entered {
            Ok(()) => Ok(TerminalGuard {
                _marker: (),
            }),
            Err(error) => {
                let _ = disable_raw_mode();
                Err(Error::Runtime(format!(
                    "failed to enter alternate screen: {error}"
                )))
            }
        };
    }
}

impl Drop for TerminalGuard {
    /// Leave the alternate screen, show the cursor, disable raw mode.
    ///
    /// Dropping cannot fail; a broken stdout cannot report its own
    /// failure, so every step is best effort.
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        return;
    }
}
