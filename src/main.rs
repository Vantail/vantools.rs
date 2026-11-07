//! CLI entry point for the `van` package and plugin manager.

use std::process::ExitCode;

use cli::tui::menu::MenuChoice;
use vantools::common::cli::{collect_arguments, to_exit_code};
use vantools::common::{Error, VanError};
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
    cli::tui::log::log_err(&message, "");
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

/// Show the start menu and print the help of the chosen command.
///
/// The menu restores the terminal before it returns, so the help text
/// is written to the normal screen.
fn run_menu() -> Result<(), Error> {
    let choice = cli::tui::menu::show_menu()?;
    cli::tui::menu::clear_screen();
    return match choice {
        MenuChoice::Quit => Ok(()),
        MenuChoice::Pack => cli::pack::print_pack_help(),
        MenuChoice::Verify => cli::verify::print_verify_help(),
        MenuChoice::Unpack => cli::unpack::print_unpack_help(),
    };
}
