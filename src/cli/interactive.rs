//! Interactive terminal mode for the `van` binary.
//!
//! Running `van` without arguments on a terminal opens the menu built
//! from the command table in [`super::commands`], and a selection opens
//! the parameter form that command describes. Submitting a form runs the
//! command through the shared progress runner: a short simulated
//! animation is drawn while the command runs on a worker thread, and its
//! result stays on screen until the user proceeds, after which the menu
//! opens again.

use std::io;
use std::time::Duration;

use crossterm::cursor::MoveTo;
use crossterm::terminal::{Clear, ClearType};

use vantools::common::Error;
use vantools::tui::form::{Form, FormOutcome};
use vantools::tui::menu::{Menu, MenuOutcome};
use vantools::tui::progress::run_task;
use vantools::tui::terminal::{
    InputEvent, LineWriter, TerminalSession, poll_input,
};

use super::commands;

/// Title of the interactive menu.
const MENU_TITLE: &str = "Select a command:";

/// Menu item that ends the session.
const QUIT_OPTION: &str = "Quit";

/// Hint line drawn above a parameter form.
const FORM_HINT: &str = "Tab: next field | Enter: run | Esc: back to menu";

/// Run the interactive interface until the user quits it.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the terminal cannot be entered,
/// drawn, or read. A failing command is displayed in the interface
/// instead, so it does not change the process exit code.
pub(super) fn run() -> Result<(), Error> {
    // The guard keeps the terminal in raw mode for the whole session:
    // commands are drawn in the interface rather than on a restored
    // screen.
    let _session = TerminalSession::enter()?;
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
        // Every selection builds a fresh form, so values are not
        // preserved between runs.
        execute(index, &values)?;
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

/// Build the interactive menu from the command table.
///
/// The table decides which items the menu holds and in which order, so a
/// new command appears without changing this module.
///
/// # Errors
///
/// Returns the [`Menu::new`] error, which the fixed title and options
/// cannot trigger.
fn build_menu() -> Result<Menu, Error> {
    let mut options: Vec<String> = Vec::new();
    for command in commands::all() {
        options.push(String::from(command.name()));
    }
    options.push(String::from(QUIT_OPTION));
    return Menu::new(MENU_TITLE, &options);
}

/// Build the parameter form of one menu index.
///
/// Returns `Ok(None)` for the `Quit` item and for any index the command
/// table does not hold, which the caller treats as the end of the
/// session.
///
/// # Errors
///
/// Returns the [`Form::new`] error, which the fixed segments and widths
/// cannot trigger.
fn build_form(index: usize) -> Result<Option<Form>, Error> {
    let command = match commands::command_at(index) {
        Some(value) => value,
        None => return Ok(None),
    };
    let segments = command.form_segments();
    let widths = command.form_widths();
    let built = Form::new(&segments, widths);
    return match built {
        Ok(form) => Ok(Some(form)),
        Err(error) => Err(error),
    };
}

/// Run the command the submitted form selected.
///
/// The command comes from the shared table in [`super::commands`], so
/// the menu, the form, the progress animation, and the result screen are
/// the same for every command. Every error the command reports, from a
/// rejected field value to a failed operation, is displayed in the
/// interface rather than returned, so the menu simply re-opens and the
/// process exit code is unchanged.
///
/// # Errors
///
/// Returns [`Error::Runtime`] only when the interface cannot be drawn or
/// input cannot be read.
fn execute(index: usize, values: &[String]) -> Result<(), Error> {
    let command = match commands::command_at(index) {
        Some(value) => value,
        None => return Ok(()),
    };
    let submitted = values.to_vec();
    let task = move || {
        return command.run(&submitted);
    };
    let messages = command.messages();
    let steps = command.steps();
    return run_task(&messages, &steps, task);
}
