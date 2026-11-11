//! Interactive terminal mode for the `van` binary.
//!
//! Running `van` without arguments on a terminal opens the menu, and a
//! selection opens the matching parameter form. Submitting a form leaves
//! the alternate screen, runs the command through the same wrapper the
//! classic CLI uses, and then returns to the menu, so the command output
//! is identical in both modes.

use std::io;
use std::time::Duration;

use crossterm::cursor::MoveTo;
use crossterm::terminal::{Clear, ClearType};

use vantools::common::Error;
use vantools::tui::form::{Form, FormOutcome};
use vantools::tui::menu::{Menu, MenuOutcome};
use vantools::tui::terminal::{
    InputEvent, LineWriter, TerminalSession, poll_input,
};

use super::output;
use super::pack;
use super::unpack;
use super::verify;

/// Title of the interactive menu.
const MENU_TITLE: &str = "Select a command:";

/// Menu items, in the order the loop routes them.
const MENU_OPTIONS: [&str; 4] = ["Pack", "Verify", "Unpack", "Quit"];

/// Hint line drawn above a parameter form.
const FORM_HINT: &str = "Tab: next field | Enter: run | Esc: back to menu";

/// Width of one form field.
///
/// Twenty-eight characters keeps the widest form, `pack` and `unpack`,
/// at 76 columns, which fits an 80-column terminal.
const FIELD_WIDTH: usize = 28;

/// Run the interactive interface until the user quits it.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the terminal cannot be entered,
/// drawn, or read. A failing command is reported on the restored screen
/// instead, so it does not change the process exit code.
pub(super) fn run() -> Result<(), Error> {
    let mut session = TerminalSession::enter()?;
    let mut menu = build_menu()?;
    loop {
        draw_menu(&menu)?;
        let selected = menu_loop(&mut menu)?;
        let index = match selected {
            None => return Ok(()),
            Some(value) => value,
        };
        let mut form = match build_form(index)? {
            None => return Ok(()),
            Some(value) => value,
        };
        draw_form(&form)?;
        let values = match form_loop(&mut form)? {
            None => continue,
            Some(value) => value,
        };
        // The command runs with the terminal fully restored, so its
        // output and its own `Ctrl+C` handling look exactly like the
        // classic CLI. Every selection builds a fresh form, so values
        // are not preserved between runs.
        drop(session);
        execute(index, &values)?;
        session = TerminalSession::enter()?;
    }
}

/// Run the menu until the user selects an item or quits.
///
/// Returns the selected index, or `None` when the user quit.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when input cannot be read or the menu
/// cannot be drawn.
fn menu_loop(menu: &mut Menu) -> Result<Option<usize>, Error> {
    loop {
        let polled = poll_input(Duration::from_millis(100))?;
        let event = match polled {
            None => continue,
            Some(value) => value,
        };
        if let InputEvent::Resize {
            ..
        } = event
        {
            draw_menu(menu)?;
            continue;
        }
        match menu.handle(event) {
            MenuOutcome::Nothing => continue,
            MenuOutcome::Redraw => {
                draw_menu(menu)?;
                continue;
            }
            MenuOutcome::Selected {
                index,
            } => return Ok(Some(index)),
            MenuOutcome::Quit => return Ok(None),
            // `MenuOutcome` belongs to another crate and is
            // `#[non_exhaustive]`, so this arm is mandatory. An outcome
            // added later leaves the menu as it is.
            _ => continue,
        }
    }
}

/// Run a parameter form until it is submitted or cancelled.
///
/// Returns the field values, or `None` when the user went back to the
/// menu.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when input cannot be read or the form
/// cannot be drawn.
fn form_loop(form: &mut Form) -> Result<Option<Vec<String>>, Error> {
    loop {
        let polled = poll_input(Duration::from_millis(100))?;
        let event = match polled {
            None => continue,
            Some(value) => value,
        };
        if let InputEvent::Resize {
            ..
        } = event
        {
            draw_form(form)?;
            continue;
        }
        match form.handle(event) {
            FormOutcome::Nothing => continue,
            FormOutcome::Redraw => {
                draw_form(form)?;
                continue;
            }
            FormOutcome::Submitted => return Ok(Some(form.values())),
            FormOutcome::Cancelled => return Ok(None),
            // `FormOutcome` belongs to another crate and is
            // `#[non_exhaustive]`, so this arm is mandatory. An outcome
            // added later leaves the form as it is.
            _ => continue,
        }
    }
}

/// Clear the screen and draw the menu from the top left corner.
///
/// Starting every draw from a cleared screen means no frame can leave
/// remnants of the previous one, which structurally prevents the
/// cascaded output the old interface could produce.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the screen cannot be cleared or the
/// menu cannot be written.
fn draw_menu(menu: &Menu) -> Result<(), Error> {
    let cleared =
        crossterm::execute!(io::stdout(), MoveTo(0, 0), Clear(ClearType::All));
    match cleared {
        Ok(()) => {}
        Err(error) => {
            return Err(draw_error("menu", &error));
        }
    }
    let mut writer = LineWriter::new(io::stdout());
    let rendered = menu.render(&mut writer);
    return match rendered {
        Ok(()) => Ok(()),
        Err(error) => Err(draw_error("menu", &error)),
    };
}

/// Clear the screen, draw the hint and the form, and place the cursor.
///
/// The hint occupies row 0 and the form row 1, so the hardware cursor
/// is positioned on the form row at the column the form reports. That
/// column is far below the `u16` limit, so the cast is safe.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the screen cannot be cleared, the
/// form cannot be written, or the cursor cannot be positioned.
fn draw_form(form: &Form) -> Result<(), Error> {
    let cleared =
        crossterm::execute!(io::stdout(), MoveTo(0, 0), Clear(ClearType::All));
    match cleared {
        Ok(()) => {}
        Err(error) => {
            return Err(draw_error("form", &error));
        }
    }
    let mut writer = LineWriter::new(io::stdout());
    let hint = writer.write_line(FORM_HINT);
    match hint {
        Ok(()) => {}
        Err(error) => {
            return Err(draw_error("form", &error));
        }
    }
    let rendered = form.render(&mut writer);
    match rendered {
        Ok(()) => {}
        Err(error) => {
            return Err(draw_error("form", &error));
        }
    }
    let placed = crossterm::execute!(
        io::stdout(),
        MoveTo(form.cursor_column() as u16, 1)
    );
    return match placed {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to position the cursor: {error}"
        ))),
    };
}

/// Build the error reported when a frame cannot be drawn.
fn draw_error(subject: &str, error: &io::Error) -> Error {
    return Error::Runtime(format!("failed to draw the {subject}: {error}"));
}

/// Build the interactive menu.
///
/// # Errors
///
/// Returns the [`Menu::new`] error, which the fixed title and options
/// cannot trigger.
fn build_menu() -> Result<Menu, Error> {
    let mut options: Vec<String> = Vec::new();
    for option in MENU_OPTIONS {
        options.push(String::from(option));
    }
    return Menu::new(MENU_TITLE, &options);
}

/// Build the parameter form of one menu index.
///
/// Returns `Ok(None)` for the `Quit` item and for any index this loop
/// does not know, which the caller treats as the end of the session.
///
/// # Errors
///
/// Returns the [`Form::new`] error, which the fixed segments and widths
/// cannot trigger.
fn build_form(index: usize) -> Result<Option<Form>, Error> {
    let segments = match index {
        0 => vec![
            String::from("van pack -s "),
            String::from(" -t "),
            String::from(""),
        ],
        1 => vec![String::from("van verify -s "), String::from("")],
        2 => vec![
            String::from("van unpack -s "),
            String::from(" -t "),
            String::from(""),
        ],
        _ => return Ok(None),
    };
    let widths = match index {
        1 => vec![FIELD_WIDTH],
        _ => vec![FIELD_WIDTH, FIELD_WIDTH],
    };
    let built = Form::new(&segments, &widths);
    return match built {
        Ok(form) => Ok(Some(form)),
        Err(error) => Err(error),
    };
}

/// Run the command a submitted form describes.
///
/// A failure is printed exactly like the classic CLI prints it, on the
/// restored screen, and does not change the process exit code, so the
/// menu simply re-opens. The caller therefore only sees the errors of
/// the surrounding terminal work.
///
/// # Errors
///
/// Returns [`Error::Runtime`] only if the command reports one, which the
/// caller must not turn into an exit code; the error is printed here.
fn execute(index: usize, values: &[String]) -> Result<(), Error> {
    let outcome = match index {
        0 => execute_pack(values),
        1 => execute_verify(values),
        2 => execute_unpack(values),
        _ => Ok(()),
    };
    return match outcome {
        Ok(()) => Ok(()),
        Err(error) => {
            let message = format!("{error}");
            output::log_err(&message, "");
            Ok(())
        }
    };
}

/// Append a required flag and its value to the argument list.
fn push_flag(args: &mut Vec<String>, flag: &str, value: &str) {
    args.push(String::from(flag));
    args.push(String::from(value));
}

/// Return true when any field value starts with a dash.
///
/// A value starting with `-` would be read by the CLI parser as a flag
/// instead of as the path the form displays.
fn values_start_with_dash(values: &[String]) -> bool {
    for value in values {
        if value.starts_with('-') {
            return true;
        }
    }
    return false;
}

/// Report the error used when a form value would be read as a flag.
///
/// The error is printed on the restored screen exactly like a failed
/// command, and the caller returns `Ok(())` so the menu re-opens. A
/// submission therefore changes neither the screen nor the exit code.
fn reject_dash_values() {
    let error =
        Error::Runtime(String::from("form values must not start with a dash"));
    let message = format!("{error}");
    output::log_err(&message, "");
}

/// Build the `pack` arguments of a submitted form and run the command.
fn execute_pack(values: &[String]) -> Result<(), Error> {
    if values_start_with_dash(values) {
        reject_dash_values();
        return Ok(());
    }
    let split = values.split_first();
    let (source, rest) = match split {
        Some(value) => value,
        None => return Err(unexpected_field_count()),
    };
    let target_split = rest.split_first();
    let (target, surplus) = match target_split {
        Some(value) => value,
        None => return Err(unexpected_field_count()),
    };
    if !surplus.is_empty() {
        return Err(unexpected_field_count());
    }
    let mut args: Vec<String> = Vec::new();
    push_flag(&mut args, "-s", source);
    push_flag(&mut args, "-t", target);
    return pack::run_cli(&args);
}

/// Build the `verify` arguments of a submitted form and run the command.
fn execute_verify(values: &[String]) -> Result<(), Error> {
    if values_start_with_dash(values) {
        reject_dash_values();
        return Ok(());
    }
    let split = values.split_first();
    let (source, surplus) = match split {
        Some(value) => value,
        None => return Err(unexpected_field_count()),
    };
    if !surplus.is_empty() {
        return Err(unexpected_field_count());
    }
    let mut args: Vec<String> = Vec::new();
    push_flag(&mut args, "-s", source);
    return verify::run_cli(&args);
}

/// Build the `unpack` arguments of a submitted form and run the command.
fn execute_unpack(values: &[String]) -> Result<(), Error> {
    if values_start_with_dash(values) {
        reject_dash_values();
        return Ok(());
    }
    let split = values.split_first();
    let (source, rest) = match split {
        Some(value) => value,
        None => return Err(unexpected_field_count()),
    };
    let target_split = rest.split_first();
    let (target, surplus) = match target_split {
        Some(value) => value,
        None => return Err(unexpected_field_count()),
    };
    if !surplus.is_empty() {
        return Err(unexpected_field_count());
    }
    let mut args: Vec<String> = Vec::new();
    push_flag(&mut args, "-s", source);
    push_flag(&mut args, "-t", target);
    return unpack::run_cli(&args);
}

/// Build the error reported when a form hands over the wrong number of
/// field values.
fn unexpected_field_count() -> Error {
    return Error::Runtime(String::from(
        "form returned an unexpected field count",
    ));
}
