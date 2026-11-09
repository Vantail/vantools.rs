//! CLI application: argument collection, dispatch, and empty-argument
//! startup.
//!
//! [`run`] is the single owner of command routing for the `van` binary.
//! It collects the process arguments, then hands the invocation to the
//! wrapper of the requested subcommand in [`super::pack`],
//! [`super::unpack`], or [`super::verify`], or to the top-level parser
//! in [`vantools::van`] for `--help`, `--version`, and unknown
//! subcommands. The subcommand wrappers keep their own argument
//! parsing; this module only decides which of them runs.

use std::io;
use std::io::IsTerminal;

use vantools::common::Error;
use vantools::common::cli::collect_arguments;
use vantools::van;

use super::output;
use super::pack;
use super::unpack;
use super::verify;

/// Name of the subcommand routed to the `pack` wrapper.
const PACK_SUBCOMMAND: &str = "pack";

/// Name of the subcommand routed to the `unpack` wrapper.
const UNPACK_SUBCOMMAND: &str = "unpack";

/// Name of the subcommand routed to the `verify` wrapper.
const VERIFY_SUBCOMMAND: &str = "verify";

/// Collect the process arguments and dispatch the requested command.
///
/// # Errors
///
/// Returns [`Error::Usage`] when the arguments are not valid Unicode
/// or name an unknown subcommand, and any error of the dispatched
/// command.
pub fn run() -> Result<(), Error> {
    let arguments = collect_arguments()?;
    return dispatch(&arguments);
}

/// Route the collected arguments to the module that owns them.
///
/// `--help`, `--version`, and unknown subcommands are left to
/// [`vantools::van::run_with_args`], which renders the top-level help
/// text and usage errors.
fn dispatch(arguments: &[String]) -> Result<(), Error> {
    let split = arguments.split_first();
    let (first, rest) = match split {
        None => return run_empty_args(),
        Some(value) => value,
    };
    if first == PACK_SUBCOMMAND {
        return pack::run_cli(rest);
    }
    if first == UNPACK_SUBCOMMAND {
        return unpack::run_cli(rest);
    }
    if first == VERIFY_SUBCOMMAND {
        return verify::run_cli(rest);
    }
    return van::run_with_args(arguments.to_vec());
}

/// Run the interactive menu without arguments, else general help.
///
/// A piped stdout has no screen to draw the menu on, so it keeps the
/// plain general help text and exit code 0 the scripts and tests
/// expect. On a terminal the logo is printed first and the menu
/// routes the choice to the matching subcommand help.
fn run_empty_args() -> Result<(), Error> {
    if !io::stdout().is_terminal() {
        return output::print_general_help();
    }
    vantools::tui::logo::print_logo();
    let options = ["Pack", "Verify", "Unpack", "Quit"];
    let selected =
        vantools::tui::select::show_menu("Select a command:", &options)?;
    return match selected {
        Some(0) => pack::run_cli(&[String::from("--help")]),
        Some(1) => verify::run_cli(&[String::from("--help")]),
        Some(2) => unpack::run_cli(&[String::from("--help")]),
        Some(_) | None => Ok(()),
    };
}
