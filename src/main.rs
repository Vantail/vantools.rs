//! CLI entry point for the `van` package and plugin manager.

use std::env;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use vantools::common::cli::{report_error, to_exit_code};
use vantools::common::command::Command;
use vantools::common::{Error, VanError};
use vantools::pack::PackCommand;
use vantools::van;

/// Name of the subcommand this binary implements directly.
const PACK_SUBCOMMAND: &str = "pack";

/// Source flag of the `pack` subcommand.
const SOURCE_FLAG: &str = "-s";

/// Target flag of the `pack` subcommand.
const TARGET_FLAG: &str = "-t";

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

/// Parse the process arguments and run the requested command.
fn run() -> Result<(), Error> {
    let arguments = collect_arguments()?;
    let first = match arguments.first() {
        Some(value) => value,
        None => return van::run_with_args(arguments),
    };
    if first != PACK_SUBCOMMAND {
        return van::run_with_args(arguments);
    }
    let command = parse_pack(arguments.as_slice())?;
    return command.run();
}

/// Collect the process arguments after the program name.
///
/// Arguments that are not valid Unicode are rejected as usage errors
/// rather than aborting the process.
fn collect_arguments() -> Result<Vec<String>, Error> {
    let mut arguments: Vec<String> = Vec::new();
    for raw_argument in env::args_os().skip(1) {
        let converted = raw_argument.into_string();
        let argument = match converted {
            Ok(value) => value,
            Err(_) => {
                return Err(Error::Usage(String::from(
                    "command-line argument is not valid Unicode",
                )));
            }
        };
        arguments.push(argument);
    }
    return Ok(arguments);
}

/// Build a [`PackCommand`] from the `pack` subcommand arguments.
fn parse_pack(arguments: &[String]) -> Result<PackCommand, Error> {
    let mut source: Option<PathBuf> = None;
    let mut target: Option<PathBuf> = None;
    let mut remaining = arguments.iter().skip(1);
    loop {
        let next = remaining.next();
        let argument = match next {
            None => break,
            Some(value) => value.as_str(),
        };
        if argument == SOURCE_FLAG {
            let value = match remaining.next() {
                Some(value) => value,
                None => return Err(missing_flag_value(SOURCE_FLAG)),
            };
            source = Some(PathBuf::from(value));
        } else if argument == TARGET_FLAG {
            let value = match remaining.next() {
                Some(value) => value,
                None => return Err(missing_flag_value(TARGET_FLAG)),
            };
            target = Some(PathBuf::from(value));
        } else {
            let rendered = argument.escape_debug();
            return Err(Error::Usage(format!("unknown argument '{rendered}'")));
        }
    }
    let resolved_source = match source {
        Some(value) => value,
        None => {
            return Err(Error::Usage(format!(
                "missing required argument {SOURCE_FLAG} <source>"
            )));
        }
    };
    let resolved_target = match target {
        Some(value) => value,
        None => {
            return Err(Error::Usage(format!(
                "missing required argument {TARGET_FLAG} <target>"
            )));
        }
    };
    return Ok(PackCommand::new(resolved_source, resolved_target));
}

/// Build the usage error for a flag without its value.
fn missing_flag_value(flag: &str) -> Error {
    return Error::Usage(format!("missing value for {flag}"));
}
