//! Colored, timestamped log lines for the `van` binary.
//!
//! Every line a subcommand prints about its own work goes through this
//! module, so terminal output keeps one shape:
//!
//! ```text
//! HH:MM:SS [TAG] Message :: Value
//! ```
//!
//! The clock is read in UTC, because the crate carries no time-zone
//! dependency. When stdout is not a terminal, the decoration is dropped
//! entirely and the plain `Message: Value` text is written instead,
//! which keeps piped output, captured logs, and CI transcripts free of
//! escape sequences.
//!
//! Writing is best-effort. A stream that cannot be written cannot report
//! its own failure either, and the caller still returns the error's exit
//! code.

use std::io;
use std::io::IsTerminal;
use std::io::Write;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

/// ANSI SGR sequence for cyan.
const CYAN: &str = "\x1b[36m";

/// ANSI SGR sequence for green.
const GREEN: &str = "\x1b[32m";

/// ANSI SGR sequence for yellow.
const YELLOW: &str = "\x1b[33m";

/// ANSI SGR sequence for red.
const RED: &str = "\x1b[31m";

/// ANSI SGR sequence for magenta.
const MAGENTA: &str = "\x1b[35m";

/// ANSI SGR sequence for dark gray.
const DARK_GRAY: &str = "\x1b[90m";

/// ANSI SGR sequence that resets every attribute.
const RESET: &str = "\x1b[0m";

/// Seconds in one day, used to wrap the clock at midnight.
const SECONDS_PER_DAY: u64 = 86_400;

/// Seconds in one hour.
const SECONDS_PER_HOUR: u64 = 3_600;

/// Seconds in one minute.
const SECONDS_PER_MINUTE: u64 = 60;

/// Destination stream of one log line.
enum Stream {
    /// Standard output.
    Output,
    /// Standard error.
    Error,
}

/// Write an informational line in cyan to stdout.
pub fn log_info(message: &str, value: &str) {
    log_line(Stream::Output, CYAN, "INFO", message, value);
}

/// Write a success line in green to stdout.
pub fn log_ok(message: &str, value: &str) {
    log_line(Stream::Output, GREEN, "OK", message, value);
}

/// Write a warning line in yellow to stderr.
pub fn log_warn(message: &str, value: &str) {
    log_line(Stream::Error, YELLOW, "WARN", message, value);
}

/// Write an error line in red to stderr.
pub fn log_err(message: &str, value: &str) {
    log_line(Stream::Error, RED, "ERR", message, value);
}

/// Write a start line in magenta to stdout.
///
/// A start line announces an operation rather than reporting a result,
/// so it carries no value.
pub fn log_start(message: &str) {
    log_line(Stream::Output, MAGENTA, "START", message, "");
}

/// Write a multi-line text block in dark gray.
///
/// A block is a code-like body rather than a log event, so it carries no
/// timestamp and no tag. On a pipe, a file, or in CI the text is written
/// unchanged so captured output stays plain.
pub fn log_block(text: &str) {
    let stdout = io::stdout();
    let decorated = stdout.is_terminal();
    let mut locked = stdout.lock();
    for line in text.lines() {
        let written = match decorated {
            true => writeln!(locked, "{DARK_GRAY}{line}{RESET}"),
            false => writeln!(locked, "{line}"),
        };
        if written.is_err() {
            return;
        }
    }
    return;
}

/// Write one log line to `stream`.
///
/// The terminal check is made on stdout, so one invocation of a command
/// either produces decorated output or plain text, never a mix of both.
fn log_line(
    stream: Stream,
    color: &str,
    tag: &str,
    message: &str,
    value: &str,
) {
    let stdout = io::stdout();
    let line = match stdout.is_terminal() {
        true => decorated_line(color, tag, message, value),
        false => plain_line(message, value),
    };
    let written = match stream {
        Stream::Output => {
            let handle = io::stdout();
            let mut locked = handle.lock();
            writeln!(locked, "{line}")
        }
        Stream::Error => {
            let handle = io::stderr();
            let mut locked = handle.lock();
            writeln!(locked, "{line}")
        }
    };
    if written.is_err() {
        // The stream that failed cannot report its own failure.
        return;
    }
    return;
}

/// Build the decorated line written to a terminal.
///
/// The value is appended only when there is one, so a start line does
/// not end with a dangling separator.
fn decorated_line(
    color: &str,
    tag: &str,
    message: &str,
    value: &str,
) -> String {
    let mut line = String::new();
    line.push_str(color);
    line.push_str(&timestamp());
    line.push_str(" [");
    line.push_str(tag);
    line.push_str("] ");
    line.push_str(message);
    if !value.is_empty() {
        line.push_str(" :: ");
        line.push_str(value);
    }
    line.push_str(RESET);
    return line;
}

/// Build the plain line written to a pipe, a file, or CI.
fn plain_line(message: &str, value: &str) -> String {
    if value.is_empty() {
        return String::from(message);
    }
    return format!("{message}: {value}");
}

/// Render the current UTC time as `HH:MM:SS`.
fn timestamp() -> String {
    let now = SystemTime::now();
    let since_epoch = match now.duration_since(UNIX_EPOCH) {
        Ok(value) => value,
        // A clock before the epoch still yields a usable timestamp.
        Err(_) => Duration::ZERO,
    };
    let seconds = since_epoch.as_secs() % SECONDS_PER_DAY;
    let hour = seconds / SECONDS_PER_HOUR;
    let minute = (seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
    let second = seconds % SECONDS_PER_MINUTE;
    return format!("{hour:02}:{minute:02}:{second:02}");
}
