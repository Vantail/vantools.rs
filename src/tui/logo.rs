//! ASCII logo for the `van` binary.
//!
//! The artwork lives in `logo.txt` and is embedded at compile time,
//! so the binary carries no runtime file dependency.

use std::io::{IsTerminal, Write};

/// Embedded ASCII logo loaded from `logo.txt`.
pub const LOGO: &str = include_str!("logo.txt");

/// Print the logo in neon green when stdout is a terminal.
///
/// Trailing blank lines from the embedded file are trimmed so the
/// crate version follows on the next line, right-aligned to the
/// longest logo line and rendered in italics. Output is skipped on
/// pipes, files, and CI so captured logs stay plain. Writing is best
/// effort: a closed stdout cannot report its own failure.
pub fn print_logo() {
    let stdout = std::io::stdout();
    if !stdout.is_terminal() {
        return;
    }
    let mut handle = stdout.lock();
    let trimmed = LOGO.trim_end();
    let _ = write!(
        handle,
        "\x1b[38;5;118m{trimmed}\x1b[0m\n{}\n",
        formatted_version()
    );
    return;
}

/// Build the right-aligned italic version line for the logo.
///
/// Padding uses the display-agnostic character count of the longest
/// logo line so the visible text ends at the logo's right edge.
pub fn formatted_version() -> String {
    let version_text = format!("version: {}", env!("CARGO_PKG_VERSION"));
    let mut version_len: usize = 0;
    for _ in version_text.chars() {
        version_len += 1;
    }
    let padding = logo_max_width().saturating_sub(version_len);
    let padding_str = " ".repeat(padding);
    return format!("{padding_str}\x1b[3m{version_text}\x1b[0m");
}

/// Measure the longest line of the embedded logo in characters.
///
/// Trailing blank lines are ignored so the version aligns with the
/// last visible row of the artwork.
pub fn logo_max_width() -> usize {
    let mut max_width: usize = 0;
    for line in LOGO.trim_end().lines() {
        let mut line_len: usize = 0;
        for _ in line.chars() {
            line_len += 1;
        }
        if line_len > max_width {
            max_width = line_len;
        }
    }
    return max_width;
}
