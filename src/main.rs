//! CLI entry point for the `van` package and plugin manager.

use std::io;
use std::io::IsTerminal;
use std::io::Write;
use std::process::ExitCode;

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

/// General help text printed when `van` runs without arguments.
const GENERAL_HELP: &str = "\
Usage: van <command> [options]

Commands:
  pack    Compress a plugin directory into an archive
  verify  Validate an archive
  unpack  Extract an archive

Run 'van <command> --help' for command-specific help.
";

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
        return run_empty_args();
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

/// Run the interactive menu without arguments, else general help.
///
/// A piped stdout has no screen to draw the menu on, so it keeps the
/// plain general help text and exit code 0 the scripts and tests
/// expect. On a terminal the logo is printed first and the menu
/// routes the choice to the matching subcommand help.
fn run_empty_args() -> Result<(), Error> {
    if !io::stdout().is_terminal() {
        return print_general_help();
    }
    vantools::tui::logo::print_logo();
    let options = ["Pack", "Verify", "Unpack", "Quit"];
    let selected =
        vantools::tui::select::show_menu("Select a command:", &options)?;
    return match selected {
        Some(0) => cli::pack::run_cli(&[String::from("--help")]),
        Some(1) => cli::verify::run_cli(&[String::from("--help")]),
        Some(2) => cli::unpack::run_cli(&[String::from("--help")]),
        Some(_) | None => Ok(()),
    };
}

/// Print the logo and the general help text to stdout.
fn print_general_help() -> Result<(), Error> {
    vantools::tui::logo::print_logo();
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let written = write!(handle, "{GENERAL_HELP}");
    return match written {
        Ok(()) => Ok(()),
        Err(error) => {
            Err(Error::Runtime(format!("failed to write output: {error}")))
        }
    };
}
