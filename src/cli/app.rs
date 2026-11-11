//! CLI application: argument collection and dispatch.
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

use super::interactive;
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
/// Without arguments the interactive menu opens when stdout is a
/// terminal, because somebody is watching, while a piped or redirected
/// stdout keeps the plain general help so scripts and CI runs are
/// unaffected. `--help`, `--version`, and unknown subcommands are left
/// to [`vantools::van::run_with_args`], which renders the top-level
/// help text and usage errors.
fn dispatch(arguments: &[String]) -> Result<(), Error> {
    let split = arguments.split_first();
    let (first, rest) = match split {
        None => {
            if io::stdout().is_terminal() {
                return interactive::run();
            }
            return output::print_general_help();
        }
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
