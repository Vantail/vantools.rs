//! Cosmetic block progress bar shared by the subcommands.
//!
//! The bar is a fixed-duration flourish: it fills a twenty-block track
//! from zero to one hundred percent so a command that takes a moment
//! does not look stuck. It reports no real progress, and it writes
//! nothing when stdout is not a terminal, which keeps redirected output
//! and CI logs free of carriage returns and escape sequences.

use std::io;
use std::io::IsTerminal;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

/// Width of the bar in blocks.
const BAR_WIDTH: usize = 20;

/// Percentage points added by each frame.
const STEP: usize = 5;

/// Time each frame stays on screen.
const FRAME_DELAY: Duration = Duration::from_millis(3);

/// Block drawn for a filled cell.
const FILLED_BLOCK: char = '█';

/// Block drawn for an empty cell.
const EMPTY_BLOCK: char = '░';

/// ANSI SGR sequence for green.
const GREEN: &str = "\x1b[32m";

/// ANSI SGR sequence for dark gray.
const DARK_GRAY: &str = "\x1b[90m";

/// ANSI SGR sequence that resets every attribute.
const RESET: &str = "\x1b[0m";

/// Width reserved to clear the finished bar, including `[`, `]`, the
/// separator, and the widest percentage.
const LINE_WIDTH: usize = BAR_WIDTH + 10;

/// Fill the bar from zero to one hundred percent once.
///
/// The animation is skipped when stdout is not a terminal. On a terminal
/// the finished line is cleared and a newline is printed, so the
/// command's real output starts on a clean line.
pub fn show_progress() {
    let stdout = io::stdout();
    if !stdout.is_terminal() {
        return;
    }
    let mut handle = stdout.lock();
    let mut percent: usize = 0;
    loop {
        let frame = format!("\r{}", render_frame(percent));
        if !emit_frame(&mut handle, &frame) {
            return;
        }
        if percent >= 100 {
            break;
        }
        sleep(FRAME_DELAY);
        percent += STEP;
    }
    let blank = format!("\r{}\r\n", " ".repeat(LINE_WIDTH));
    emit_frame(&mut handle, &blank);
    return;
}

/// Write one frame and report whether the stream accepted it.
///
/// A closed or broken stdout makes the write fail; the caller then stops
/// the animation instead of panicking through `print!`.
fn emit_frame(handle: &mut impl Write, frame: &str) -> bool {
    let written = handle.write_all(frame.as_bytes());
    if written.is_err() {
        return false;
    }
    let flushed = handle.flush();
    return flushed.is_ok();
}

/// Build one frame for `percent`.
///
/// The filled part is green and the remaining part is dark gray, so the
/// bar reads as filled even on a dark terminal. The percentage is not
/// padded: it only grows as the bar fills, so a longer frame always
/// overwrites a shorter one.
fn render_frame(percent: usize) -> String {
    let filled = percent * BAR_WIDTH / 100;
    let empty = BAR_WIDTH - filled;
    let mut frame = String::from("[");
    frame.push_str(GREEN);
    for _ in 0..filled {
        frame.push(FILLED_BLOCK);
    }
    frame.push_str(RESET);
    frame.push_str(DARK_GRAY);
    for _ in 0..empty {
        frame.push(EMPTY_BLOCK);
    }
    frame.push_str(RESET);
    frame.push_str(&format!("] {percent}%"));
    return frame;
}
