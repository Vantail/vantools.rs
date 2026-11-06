//! Startup banner for the `van` binary.
//!
//! The banner is the `VAN TOOLS` wordmark in the ANSI Shadow figlet
//! face. It is written only to a terminal: a redirected run, a pipe, or
//! a CI job gets no escape sequences and no banner, so captured output
//! stays plain.

use std::io;
use std::io::IsTerminal;
use std::io::Write;

/// ANSI SGR sequence for neon green.
const NEON_GREEN: &str = "\x1b[38;5;118m";

/// ANSI SGR sequence that resets every attribute.
const RESET: &str = "\x1b[0m";

/// The `VAN TOOLS` wordmark, one entry per row.
///
/// The rows are kept verbatim: the shadow face depends on the exact
/// column of every block and box-drawing character.
const LOGO_ROWS: [&str; 6] = [
    "██╗   ██╗ █████╗███╗   ██╗████████╗ ██████╗ ██████╗██╗███████╗",
    "██║   ██║██╔══██╗████╗  ██║╚══██╔══╝██╔═══██╗██╔═══██╗██║██╔════╝",
    "██║   ██║███████║██╔██╗ ██║   ██║██║   ██║██║   ██║██║███████╗",
    "╚██╗ ██╔╝██╔══██║██║╚██╗██║   ██║██║   ██║██║   ██║██║╚════██║",
    " ╚████╔╝██║  ██║██║ ╚████║   ██║╚██████╔╝╚██████╔╝███████╗███████║",
    "  ╚═══╝╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝ ╚═════╝ ╚═════╝╚══════╝╚══════╝",
];

/// Print the `VAN TOOLS` wordmark in neon green.
///
/// Nothing is written when stdout is not a terminal.
pub fn print_logo() {
    let stdout = io::stdout();
    if !stdout.is_terminal() {
        return;
    }
    let handle = io::stdout();
    let mut locked = handle.lock();
    for row in LOGO_ROWS {
        let written = writeln!(locked, "{NEON_GREEN}{row}{RESET}");
        if written.is_err() {
            return;
        }
    }
    return;
}
