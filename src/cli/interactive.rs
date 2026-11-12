//! Interactive terminal mode for the `van` binary.
//!
//! Running `van` without arguments on a terminal opens the menu, and a
//! selection opens the matching parameter form. Submitting a form runs
//! the command inside the interface: a short simulated progress
//! animation is drawn while the command runs on a worker thread, and its
//! result stays on screen until the user proceeds, after which the menu
//! opens again.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossterm::cursor::MoveTo;
use crossterm::terminal::{Clear, ClearType};

use vantools::common::Error;
use vantools::common::validation::{escape_path, escape_text};
use vantools::pack::PackCommand;
use vantools::tui::form::{Form, FormOutcome};
use vantools::tui::menu::{Menu, MenuOutcome};
use vantools::tui::progress::{ProgressStep, run_task};
use vantools::tui::terminal::{
    InputEvent, LineWriter, TerminalSession, poll_input,
};
use vantools::unpack::UnpackCommand;
use vantools::verify::VerifyCommand;

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

/// Heading of the `pack` progress display.
const PACK_TITLE: &str = "Packing plugin";

/// Heading of the `verify` progress display.
const VERIFY_TITLE: &str = "Verifying archive";

/// Heading of the `unpack` progress display.
const UNPACK_TITLE: &str = "Unpacking plugin";

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
/// Every command goes through [`run_task`], which draws the simulated
/// progress animation while the command runs on a worker thread and
/// keeps its result on screen until the user presses a key. A failed
/// command is displayed in the interface rather than returned, so the
/// menu simply re-opens and the process exit code is unchanged.
///
/// # Errors
///
/// Returns [`Error::Runtime`] only when the interface cannot be drawn or
/// input cannot be read.
fn execute(index: usize, values: &[String]) -> Result<(), Error> {
    return match index {
        0 => execute_pack(values),
        1 => execute_verify(values),
        2 => execute_unpack(values),
        _ => Ok(()),
    };
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

/// Build the error shown for a form value that starts with a dash.
///
/// Such a value would be read by the CLI parser as a flag instead of as
/// the path the form displays, so the command is not run.
fn dash_value_error() -> Error {
    return Error::Runtime(String::from(
        "form values must not start with a dash",
    ));
}

/// Run a submitted `pack` form in the interface.
fn execute_pack(values: &[String]) -> Result<(), Error> {
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
    let rejected = values_start_with_dash(values);
    let source = PathBuf::from(source.as_str());
    let target = PathBuf::from(target.as_str());
    let task = move || {
        if rejected {
            return Err(dash_value_error());
        }
        return run_pack(&source, &target);
    };
    return run_task(PACK_TITLE, &pack_steps(), task);
}

/// Run a submitted `verify` form in the interface.
fn execute_verify(values: &[String]) -> Result<(), Error> {
    let split = values.split_first();
    let (source, surplus) = match split {
        Some(value) => value,
        None => return Err(unexpected_field_count()),
    };
    if !surplus.is_empty() {
        return Err(unexpected_field_count());
    }
    let rejected = values_start_with_dash(values);
    let source = PathBuf::from(source.as_str());
    let task = move || {
        if rejected {
            return Err(dash_value_error());
        }
        return run_verify(&source);
    };
    return run_task(VERIFY_TITLE, &verify_steps(), task);
}

/// Run a submitted `unpack` form in the interface.
fn execute_unpack(values: &[String]) -> Result<(), Error> {
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
    let rejected = values_start_with_dash(values);
    let source = PathBuf::from(source.as_str());
    let target = PathBuf::from(target.as_str());
    let task = move || {
        if rejected {
            return Err(dash_value_error());
        }
        return run_unpack(&source, &target);
    };
    return run_task(UNPACK_TITLE, &unpack_steps(), task);
}

/// Build the error reported when a form hands over the wrong number of
/// field values.
fn unexpected_field_count() -> Error {
    return Error::Runtime(String::from(
        "form returned an unexpected field count",
    ));
}

/// Steps shown while a plugin is packed.
fn pack_steps() -> Vec<ProgressStep> {
    return vec![
        ProgressStep::new(20, "Preparing engine..."),
        ProgressStep::new(40, "Checking plugin configuration..."),
        ProgressStep::new(60, "Packing plugin files..."),
        ProgressStep::new(80, "Finalizing package..."),
        ProgressStep::new(100, "Ready to roll!"),
    ];
}

/// Steps shown while an archive is verified.
fn verify_steps() -> Vec<ProgressStep> {
    return vec![
        ProgressStep::new(25, "Popping the hood..."),
        ProgressStep::new(50, "Inspecting archive structure..."),
        ProgressStep::new(75, "Checking manifest and entrypoint..."),
        ProgressStep::new(100, "Roadworthiness confirmed!"),
    ];
}

/// Steps shown while an archive is unpacked.
fn unpack_steps() -> Vec<ProgressStep> {
    return vec![
        ProgressStep::new(20, "Preparing engine..."),
        ProgressStep::new(40, "Reading archive..."),
        ProgressStep::new(60, "Unpacking plugin files..."),
        ProgressStep::new(80, "Installing plugin..."),
        ProgressStep::new(100, "Ready to roll!"),
    ];
}

/// Run `pack` and describe what it produced.
fn run_pack(source: &Path, target: &Path) -> Result<String, Error> {
    let command = PackCommand::new(source.to_path_buf(), target.to_path_buf());
    let report = command.run()?;
    let mut lines: Vec<String> = Vec::new();
    lines.push(String::from("Plugin successfully packed."));
    if report.target_created {
        let created = escape_path(target);
        lines.push(format!("Target directory created: {created}"));
    }
    if report.overwrote_existing {
        let archive = escape_path(&report.archive_path);
        lines.push(format!("Existing archive overwritten: {archive}"));
    }
    let archive = escape_path(&report.archive_path);
    lines.push(format!("Archive: {archive}"));
    lines.push(format!("Files written: {}", report.files_written));
    let ignored = report.ignored_files.len();
    lines.push(format!("Files ignored: {ignored}"));
    return Ok(lines.join("\n"));
}

/// Run `verify` and describe what it found.
fn run_verify(source: &Path) -> Result<String, Error> {
    let command = VerifyCommand::new(source.to_path_buf());
    let report = command.run()?;
    let mut lines: Vec<String> = Vec::new();
    lines.push(String::from("Archive verified successfully."));
    lines.push(format!("Archive: {}", escape_path(&report.source)));
    let name = escape_text(&report.manifest.name);
    let version = escape_text(&report.manifest.version);
    lines.push(format!("Plugin: {name} {version}"));
    lines.push(format!("Entries: {}", report.entry_names.len()));
    return Ok(lines.join("\n"));
}

/// Run `unpack` and describe what it installed.
fn run_unpack(source: &Path, target: &Path) -> Result<String, Error> {
    let command =
        UnpackCommand::new(source.to_path_buf(), target.to_path_buf());
    let report = command.run()?;
    let mut lines: Vec<String> = Vec::new();
    lines.push(String::from("Plugin successfully unpacked."));
    lines.push(format!("Archive: {}", escape_path(&report.archive)));
    lines.push(format!("Installed to: {}", escape_path(&report.target)));
    lines.push(format!("Files written: {}", report.files_written));
    return Ok(lines.join("\n"));
}
