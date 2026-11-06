//! Cosmetic progress animation shared by the subcommands.
//!
//! The animation is a fixed-duration flourish: it drives a small ASCII
//! car across a twenty-cell track so a command that takes a moment does
//! not look stuck. It reports no real progress, and it writes nothing
//! when stdout is not a terminal, which keeps redirected output and CI
//! logs free of carriage returns and escape sequences.

use std::io;
use std::io::IsTerminal;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

/// Width of the track in characters, including the cells the car covers.
const TRACK_WIDTH: usize = 20;

/// The car driven across the track.
const CAR: &str = "[o-o]";

/// Width of the car in characters.
const CAR_WIDTH: usize = 5;

/// Percentage points advanced by each frame.
const STEP: usize = 5;

/// Time each frame stays on screen.
const FRAME_DELAY: Duration = Duration::from_millis(30);

/// Width of the line the animation occupies, used to clear it.
const LINE_WIDTH: usize = TRACK_WIDTH + 5;

/// Drive the car across the track once.
///
/// The animation is skipped when stdout is not a terminal, so piping the
/// command or running it in CI leaves the output untouched. On a
/// terminal the finished line is cleared and a newline is printed, so
/// the command's real output starts on a clean line.
pub fn show_progress() {
    let stdout = io::stdout();
    if !stdout.is_terminal() {
        return;
    }
    let mut percent: usize = 0;
    while percent <= 100 {
        print!("{}", render_frame(percent));
        let flushed = io::stdout().flush();
        if flushed.is_err() {
            return;
        }
        if percent == 100 {
            break;
        }
        sleep(FRAME_DELAY);
        percent += STEP;
    }
    let blank = " ".repeat(LINE_WIDTH);
    print!("\r{blank}\r\n");
    return;
}

/// Build one animation frame for `percent`.
///
/// The car is placed so that its left edge covers the track cell nearest
/// to `percent` while staying inside the track at both ends.
fn render_frame(percent: usize) -> String {
    let offset = percent * (TRACK_WIDTH - CAR_WIDTH) / 100;
    let mut frame = String::with_capacity(TRACK_WIDTH + 6);
    frame.push('\r');
    for _ in 0..offset {
        frame.push('.');
    }
    frame.push_str(CAR);
    let trailing = TRACK_WIDTH - CAR_WIDTH - offset;
    for _ in 0..trailing {
        frame.push('.');
    }
    frame.push_str(&format!("{percent:4}%"));
    return frame;
}
