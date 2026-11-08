//! CLI entry point for the `van` package and plugin manager.

use std::path::PathBuf;
use std::process::ExitCode;

use vantools::common::cli::{collect_arguments, to_exit_code};
use vantools::common::{Error, VanError};
use vantools::tui::menu::MenuChoice;
use vantools::van;

mod cli;

/// Name of the subcommand routed to the `pack` module.
const PACK_SUBCOMMAND: &str = "pack";

/// Name of the subcommand routed to the `unpack` module.
const UNPACK_SUBCOMMAND: &str = "unpack";

/// Name of the subcommand routed to the `verify` module.
const VERIFY_SUBCOMMAND: &str = "verify";

fn main() -> ExitCode {
    let result = run();
    let error = match result {
        Ok(()) => {
            return ExitCode::SUCCESS;
        }
        Err(error) => error,
    };
    // The rendered error already starts with `error: `, so it becomes
    // the log message rather than repeating that prefix as a value.
    let message = format!("{error}");
    vantools::tui::log::log_err(&message, "");
    let code: i32 = error.exit_code();
    return ExitCode::from(to_exit_code(code));
}

/// Parse the process arguments and route to the requested subcommand.
fn run() -> Result<(), Error> {
    let arguments = collect_arguments()?;
    if arguments.is_empty() {
        return run_menu();
    }
    let split = arguments.split_first();
    let (first, rest) = match split {
        Some(value) => value,
        None => return van::run_with_args(arguments),
    };
    if first == PACK_SUBCOMMAND {
        return cli::pack::run_cli(rest);
    }
    if first == UNPACK_SUBCOMMAND {
        return cli::unpack::run_cli(rest);
    }
    if first == VERIFY_SUBCOMMAND {
        return cli::verify::run_cli(rest);
    }
    return van::run_with_args(arguments);
}

/// Show the start menu and run the chosen command.
///
/// The menu restores the terminal before it returns, so the form and the
/// command output are written to the normal screen. `pack` asks for its
/// paths in the interactive form; the other commands print their help
/// text.
fn run_menu() -> Result<(), Error> {
    let choice = vantools::tui::menu::show_menu()?;
    vantools::tui::menu::clear_screen();
    return match choice {
        MenuChoice::Quit => Ok(()),
        MenuChoice::Pack => run_pack_form(),
        MenuChoice::Verify => cli::verify::print_verify_help(),
        MenuChoice::Unpack => cli::unpack::print_unpack_help(),
        // The library enum is non-exhaustive, so a variant added later
        // must not break the binary; leaving the menu is the safe default.
        _ => Ok(()),
    };
}

/// Collect the `pack` paths in the interactive form and run the command.
///
/// The screen is cleared once the form has closed, so the progress bar
/// and the report start on an empty screen. Cancelling the form ends the
/// run successfully without doing any work.
fn run_pack_form() -> Result<(), Error> {
    let values = vantools::tui::form::show_pack_form()?;
    vantools::tui::menu::clear_screen();
    let (source, target) = match values {
        Some(values) => values,
        None => return Ok(()),
    };
    return cli::pack::run_with_paths(
        PathBuf::from(source),
        PathBuf::from(target),
    );
}
