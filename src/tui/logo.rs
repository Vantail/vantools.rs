//! ASCII logo for the `van` binary.
//!
//! The artwork lives in `logo.txt` and is embedded at compile time,
//! so the binary carries no runtime file dependency.

use std::io::{IsTerminal, Write};

/// Embedded ASCII logo loaded from `logo.txt`.
const LOGO: &str = include_str!("logo.txt");

/// Print the logo in neon green when stdout is a terminal.
///
/// Output is skipped on pipes, files, and CI so captured logs stay
/// plain. Writing is best effort: a closed stdout cannot report its
/// own failure.
pub fn print_logo() {
    let stdout = std::io::stdout();
    if !stdout.is_terminal() {
        return;
    }
    let mut handle = stdout.lock();
    let _ = writeln!(handle, "\x1b[38;5;118m{LOGO}\x1b[0m");
    return;
}

#[cfg(test)]
mod tests {
    use super::LOGO;

    #[test]
    fn logo_is_not_empty() {
        assert!(!LOGO.is_empty(), "embedded logo is empty");
        return;
    }

    #[test]
    fn logo_contains_underscore() {
        assert!(LOGO.contains('_'), "embedded logo is missing '_'");
        return;
    }
}
