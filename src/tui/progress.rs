//! Simulated progress and result feedback for interactive commands.
//!
//! The interactive interface runs every command through [`run_task`]. The
//! command itself runs on a worker thread while this module draws a
//! short, deliberately simulated progress animation on the caller's
//! thread, so the animation stays responsive and a fast command still
//! shows every step. A successful result is shown after the animation
//! reached its last step; a failure stops the animation at the step it
//! reached and is shown immediately. Either result stays on screen until
//! the user presses a key.

use std::io;
use std::io::Write;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;
use std::time::Instant;

use crossterm::cursor::MoveTo;
use crossterm::terminal::{Clear, ClearType};

use crate::common::Error;
use crate::tui::terminal::{InputEvent, LineWriter, poll_input};

/// Heading drawn above the progress bar.
const HEADING: &str = "[ van ]";

/// Number of cells in the progress bar.
const BAR_WIDTH: usize = 20;

/// Delay every simulated step is shown for.
const STEP_DELAY: Duration = Duration::from_millis(200);

/// Poll interval used while waiting for the user to dismiss a result.
const DISMISS_POLL: Duration = Duration::from_millis(100);

/// Title of a successful result.
const SUCCESS_TITLE: &str = "✓ Pit stop complete!";

/// Title of a failed result.
const FAILURE_TITLE: &str = "✗ Engine stalled!";

/// One simulated step of the progress animation.
///
/// The step does not describe real progress: its percentage and message
/// are predefined by the command and shown for [`STEP_DELAY`].
#[non_exhaustive]
pub struct ProgressStep {
    /// Completion percentage the bar shows for this step.
    pub percent: usize,
    /// Status message drawn below the bar.
    pub message: String,
}

impl ProgressStep {
    /// Build a progress step for the given percentage and message.
    pub fn new(percent: usize, message: &str) -> Self {
        return ProgressStep {
            percent,
            message: String::from(message),
        };
    }
}

/// Result drawn below the progress bar.
enum ResultBlock<'a> {
    /// A completed command and its description.
    Success {
        /// Lines describing what the command produced.
        description: &'a str,
    },
    /// A failed command and the error it reported.
    Failure {
        /// The error the command reported.
        error: &'a Error,
    },
}

/// What the animation knows about a task when it stops drawing steps.
struct AnimationState {
    /// Index of the step drawn last.
    last_index: usize,
    /// The description of a task that already succeeded.
    success: Option<String>,
    /// The error of a task that failed.
    failure: Option<Error>,
}

/// The outcome of waiting for a task during one simulated step.
enum StepWait {
    /// The step elapsed without a result.
    Pending,
    /// The task succeeded with this description.
    Success(String),
    /// The task failed with this error.
    Failure(Error),
    /// The worker stopped without reporting a result.
    Closed,
}

/// Run `task` while showing `steps`, then show its result until the user
/// presses a key.
///
/// The task runs on a worker thread, so the animation keeps drawing even
/// when the task finishes at once, and the command is never slowed down
/// by the animation. A successful task always shows every step; a failed
/// task stops the animation at the step it reached and its reason is
/// displayed immediately.
///
/// Returns only the errors of drawing and reading input. A task failure
/// is displayed in the interface rather than returned.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the screen cannot be drawn or input
/// cannot be read, and when `steps` is empty.
pub fn run_task<F>(
    title: &str,
    steps: &[ProgressStep],
    task: F,
) -> Result<(), Error>
where
    F: FnOnce() -> Result<String, Error> + Send + 'static,
{
    if steps.is_empty() {
        return Err(Error::Runtime(String::from(
            "progress requires at least one step",
        )));
    }
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let result = task();
        // The receiver is only gone when the interface itself failed,
        // so there is nobody left to report the result to.
        let _ = sender.send(result);
    });
    let mut state = animate(title, steps, &receiver)?;
    return finish(title, steps, &mut state, &receiver, worker);
}

/// Draw every step, watching the task so a failure stops the animation.
fn animate(
    title: &str,
    steps: &[ProgressStep],
    receiver: &Receiver<Result<String, Error>>,
) -> Result<AnimationState, Error> {
    let mut state = AnimationState {
        last_index: 0,
        success: None,
        failure: None,
    };
    for (index, step) in steps.iter().enumerate() {
        state.last_index = index;
        draw_frame(title, step, None)?;
        // A success that arrived early keeps the animation going, so a
        // fast command still shows the complete progress.
        if state.success.is_some() {
            thread::sleep(STEP_DELAY);
            continue;
        }
        match wait_for_step(receiver) {
            StepWait::Pending => {}
            StepWait::Success(description) => {
                state.success = Some(description);
            }
            StepWait::Failure(error) => {
                state.failure = Some(error);
                break;
            }
            StepWait::Closed => {
                state.failure = Some(closed_error());
                break;
            }
        }
    }
    return Ok(state);
}

/// Show the task result, waiting for one that outlived the animation.
fn finish(
    title: &str,
    steps: &[ProgressStep],
    state: &mut AnimationState,
    receiver: &Receiver<Result<String, Error>>,
    worker: thread::JoinHandle<()>,
) -> Result<(), Error> {
    let step = match steps.get(state.last_index) {
        Some(value) => value,
        None => {
            return Err(Error::Runtime(String::from(
                "progress lost track of its current step",
            )));
        }
    };
    if state.success.is_none() && state.failure.is_none() {
        record_outcome(state, receiver);
    }
    // The worker has sent its result in every remaining case, so this
    // join cannot wait for work the user is still watching.
    let _ = worker.join();
    let failure = match state.failure.take() {
        Some(value) => value,
        None => {
            let description = state.success.take().unwrap_or_default();
            return show_success(title, step, &description);
        }
    };
    return show_failure(title, step, &failure);
}

/// Record the result of a task that outlived the animation.
fn record_outcome(
    state: &mut AnimationState,
    receiver: &Receiver<Result<String, Error>>,
) {
    match receiver.recv() {
        Ok(Ok(description)) => state.success = Some(description),
        Ok(Err(error)) => state.failure = Some(error),
        Err(_) => state.failure = Some(closed_error()),
    }
}

/// Wait up to one step for the task result.
///
/// A success is returned only after the full step delay, so the step
/// that received it stays visible; a failure is returned at once.
fn wait_for_step(receiver: &Receiver<Result<String, Error>>) -> StepWait {
    let deadline = Instant::now() + STEP_DELAY;
    return match receiver.recv_timeout(STEP_DELAY) {
        Ok(Ok(description)) => {
            hold_until(deadline);
            StepWait::Success(description)
        }
        Ok(Err(error)) => StepWait::Failure(error),
        Err(RecvTimeoutError::Timeout) => StepWait::Pending,
        Err(RecvTimeoutError::Disconnected) => StepWait::Closed,
    };
}

/// Sleep until `deadline`, if it has not passed yet.
fn hold_until(deadline: Instant) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if !remaining.is_zero() {
        thread::sleep(remaining);
    }
}

/// Draw a successful result on top of the last progress frame.
fn show_success(
    title: &str,
    step: &ProgressStep,
    description: &str,
) -> Result<(), Error> {
    let block = ResultBlock::Success {
        description,
    };
    draw_frame(title, step, Some(&block))?;
    return wait_for_dismiss(title, step, &block);
}

/// Draw a failed result on top of the progress frame that was reached.
fn show_failure(
    title: &str,
    step: &ProgressStep,
    error: &Error,
) -> Result<(), Error> {
    let block = ResultBlock::Failure {
        error,
    };
    draw_frame(title, step, Some(&block))?;
    return wait_for_dismiss(title, step, &block);
}

/// Keep the result on screen until the user proceeds.
fn wait_for_dismiss(
    title: &str,
    step: &ProgressStep,
    result: &ResultBlock,
) -> Result<(), Error> {
    loop {
        let polled = poll_input(DISMISS_POLL)?;
        let event = match polled {
            None => continue,
            Some(value) => value,
        };
        if let InputEvent::Resize {
            ..
        } = event
        {
            draw_frame(title, step, Some(result))?;
            continue;
        }
        return Ok(());
    }
}

/// Clear the screen and draw one progress frame.
fn draw_frame(
    title: &str,
    step: &ProgressStep,
    result: Option<&ResultBlock>,
) -> Result<(), Error> {
    let cleared =
        crossterm::execute!(io::stdout(), MoveTo(0, 0), Clear(ClearType::All));
    match cleared {
        Ok(()) => {}
        Err(error) => return Err(draw_error(&error)),
    }
    let mut writer = LineWriter::new(io::stdout());
    let drawn = draw_body(title, &mut writer, step, result);
    match drawn {
        Ok(()) => {}
        Err(error) => return Err(draw_error(&error)),
    }
    let flushed = writer.flush();
    return match flushed {
        Ok(()) => Ok(()),
        Err(error) => Err(draw_error(&error)),
    };
}

/// Write the heading, the bar, the status, and an optional result.
fn draw_body<W: Write>(
    title: &str,
    writer: &mut LineWriter<W>,
    step: &ProgressStep,
    result: Option<&ResultBlock>,
) -> io::Result<()> {
    writer.write_line(&format!("{HEADING} {title}"))?;
    writer.write_line("")?;
    writer.write_line(&bar_text(step.percent))?;
    writer.write_line(&step.message)?;
    let shown = match result {
        Some(value) => value,
        None => return Ok(()),
    };
    writer.write_line("")?;
    return write_result(writer, shown);
}

/// Write the result block that follows a progress frame.
fn write_result<W: Write>(
    writer: &mut LineWriter<W>,
    result: &ResultBlock,
) -> io::Result<()> {
    return match result {
        ResultBlock::Success {
            description,
        } => {
            writer.write_line(SUCCESS_TITLE)?;
            writer.write_block(description)
        }
        ResultBlock::Failure {
            error,
        } => {
            writer.write_line(FAILURE_TITLE)?;
            let reason = reason_of(error);
            writer.write_line(&format!("Reason: {reason}"))
        }
    };
}

/// Render the progress bar for `percent`, clamped to its range.
fn bar_text(percent: usize) -> String {
    let bounded = percent.min(100);
    let filled = BAR_WIDTH * bounded / 100;
    let mut bar = String::from("[");
    let mut drawn = 0;
    while drawn < BAR_WIDTH {
        if drawn < filled {
            bar.push('#');
        } else {
            bar.push('-');
        }
        drawn += 1;
    }
    bar.push(']');
    bar.push(' ');
    bar.push_str(&bounded.to_string());
    bar.push('%');
    return bar;
}

/// Render an error without the `error: ` prefix of its display form.
fn reason_of(error: &Error) -> String {
    let text = format!("{error}");
    return match text.strip_prefix("error: ") {
        Some(value) => String::from(value),
        None => text,
    };
}

/// Build the error reported when the screen cannot be drawn.
fn draw_error(error: &io::Error) -> Error {
    return Error::Runtime(format!("failed to draw command progress: {error}"));
}

/// Build the error reported when the worker stops without a result.
fn closed_error() -> Error {
    return Error::Runtime(String::from(
        "the command stopped without reporting a result",
    ));
}

#[cfg(test)]
mod tests {
    use super::{bar_text, reason_of};
    use crate::common::Error;

    #[test]
    fn bar_fills_the_requested_share() {
        assert_eq!(bar_text(40), "[########------------] 40%");
    }

    #[test]
    fn bar_is_empty_at_zero() {
        assert_eq!(bar_text(0), "[--------------------] 0%");
    }

    #[test]
    fn bar_is_full_at_one_hundred() {
        assert_eq!(bar_text(100), "[####################] 100%");
    }

    #[test]
    fn bar_clamps_percent_above_one_hundred() {
        assert_eq!(bar_text(150), "[####################] 100%");
    }

    #[test]
    fn reason_strips_the_shared_error_prefix() {
        let error = Error::Runtime(String::from("Permission denied."));
        assert_eq!(reason_of(&error), "Permission denied.");
    }
}
