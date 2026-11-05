//! CLI entry point for the `van` package and plugin manager.

use std::io;
use std::process::ExitCode;

use vantools::common::cli::{collect_arguments, report_error, to_exit_code};
use vantools::common::{Error, VanError};
use vantools::van;

/// Name of the subcommand routed to the `pack` module.
const PACK_SUBCOMMAND: &str = "pack";

fn main() -> ExitCode {
    let result = run();
    let error = match result {
        Ok(()) => {
            return ExitCode::SUCCESS;
        }
        Err(error) => error,
    };
    let stderr = io::stderr();
    let mut handle = stderr.lock();
    report_error(&mut handle, &error);
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
        return vantools::pack::run_cli(rest);
    }
    return van::run_with_args(arguments);
}
