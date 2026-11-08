//! CLI entry point for the `van` package and plugin manager.

use std::path::PathBuf;
use std::process::ExitCode;

use vantools::common::cli::{collect_arguments, to_exit_code};
use vantools::common::{Error, VanError};
use vantools::tui::menu::{MenuChoice, MenuSelection};
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
/// The menu restores the terminal before it returns, so the command
/// output is written to the normal screen. A command chosen from the
/// list carries the paths typed into its form; quitting carries none.
fn run_menu() -> Result<(), Error> {
    let selection = vantools::tui::menu::show_menu()?;
    vantools::tui::menu::clear_screen();
    return match selection.choice() {
        MenuChoice::Quit => Ok(()),
        MenuChoice::Pack => run_pack(&selection),
        MenuChoice::Unpack => run_unpack(&selection),
        MenuChoice::Verify => run_verify(&selection),
        // The library enum is non-exhaustive, so a variant added later
        // must not break the binary; leaving the menu is the safe default.
        _ => Ok(()),
    };
}

/// Return the source and target paths of a two-field submission.
fn two_paths(selection: &MenuSelection) -> Option<(PathBuf, PathBuf)> {
    return match (selection.source(), selection.target()) {
        (Some(source), Some(target)) => {
            Some((PathBuf::from(source), PathBuf::from(target)))
        }
        _ => None,
    };
}

/// Run `pack` with the paths collected by the form.
fn run_pack(selection: &MenuSelection) -> Result<(), Error> {
    let paths = match two_paths(selection) {
        Some(value) => value,
        None => return Ok(()),
    };
    let (source, target) = paths;
    return cli::pack::run_with_paths(source, target);
}

/// Run `unpack` with the paths collected by the form.
fn run_unpack(selection: &MenuSelection) -> Result<(), Error> {
    let paths = match two_paths(selection) {
        Some(value) => value,
        None => return Ok(()),
    };
    let (source, target) = paths;
    return cli::unpack::run_with_paths(source, target);
}

/// Run `verify` with the source collected by the form.
fn run_verify(selection: &MenuSelection) -> Result<(), Error> {
    let source = match selection.source() {
        Some(value) => value,
        None => return Ok(()),
    };
    return cli::verify::run_with_path(PathBuf::from(source));
}
