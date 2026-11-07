//! CLI entry point for the `van` package and plugin manager.

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
