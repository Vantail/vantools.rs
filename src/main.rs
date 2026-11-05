//! CLI entry point for the `van` package and plugin manager.

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use vantools::common::cli::{
    CliCommand, collect_arguments, report_error, to_exit_code, version_line,
};
use vantools::common::command::Command;
use vantools::common::output::write_line;
use vantools::common::validation::escape_text;
use vantools::common::{Error, VanError};
use vantools::pack::PackCommand;
use vantools::van;

/// Name of the subcommand this binary implements directly.
const PACK_SUBCOMMAND: &str = "pack";

/// Source flag of the `pack` subcommand.
const SOURCE_FLAG: &str = "-s";

/// Target flag of the `pack` subcommand.
const TARGET_FLAG: &str = "-t";

/// Long form of the source flag of the `pack` subcommand.
const SOURCE_LONG_FLAG: &str = "--source";

/// Long form of the target flag of the `pack` subcommand.
const TARGET_LONG_FLAG: &str = "--target";

/// Short flag that requests help for the `pack` subcommand.
const HELP_FLAG: &str = "-h";

/// Long flag that requests help for the `pack` subcommand.
const HELP_LONG_FLAG: &str = "--help";

/// Short flag that requests the version of the `pack` subcommand.
const VERSION_FLAG: &str = "-v";

/// Long flag that requests the version of the `pack` subcommand.
const VERSION_LONG_FLAG: &str = "--version";

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
    return run_pack(arguments.as_slice());
}

/// Parse the `pack` subcommand arguments and run the command.
///
/// Help and version flags are answered with exit code 0. Unknown or
/// incomplete arguments produce a [`Error::CliError`] and exit code 1.
fn run_pack(arguments: &[String]) -> Result<(), Error> {
    let metadata = PackCommand::new(PathBuf::new(), PathBuf::new());
    let mut source: Option<PathBuf> = None;
    let mut target: Option<PathBuf> = None;
    let mut remaining = arguments.iter().skip(1);
    loop {
        let next = remaining.next();
        let argument = match next {
            None => break,
            Some(value) => value.as_str(),
        };
        if argument == HELP_FLAG || argument == HELP_LONG_FLAG {
            return print_line(metadata.help());
        }
        if argument == VERSION_FLAG || argument == VERSION_LONG_FLAG {
            return print_version();
        }
        let is_source = argument == SOURCE_FLAG || argument == SOURCE_LONG_FLAG;
        if is_source {
            let value = match remaining.next() {
                Some(value) => value,
                None => return Err(missing_arguments()),
            };
            source = Some(PathBuf::from(value));
            continue;
        }
        let is_target = argument == TARGET_FLAG || argument == TARGET_LONG_FLAG;
        if is_target {
            let value = match remaining.next() {
                Some(value) => value,
                None => return Err(missing_arguments()),
            };
            target = Some(PathBuf::from(value));
            continue;
        }
        return Err(unknown_argument(argument));
    }
    let resolved_source = match source {
        Some(value) => value,
        None => return Err(missing_arguments()),
    };
    let resolved_target = match target {
        Some(value) => value,
        None => return Err(missing_arguments()),
    };
    let command = PackCommand::new(resolved_source, resolved_target);
    return command.run();
}

/// Build the error for an argument the `pack` command does not accept.
fn unknown_argument(argument: &str) -> Error {
    let rendered = escape_text(argument);
    return Error::CliError(format!(
        "Unknown argument: {rendered}. Run 'van pack --help' for usage."
    ));
}

/// Build the error for a `pack` invocation without source and target.
fn missing_arguments() -> Error {
    return Error::CliError(String::from(
        "Missing required arguments. Run 'van pack --help' for usage.",
    ));
}

/// Print one line to stdout.
fn print_line(text: &str) -> Result<(), Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    return write_line(&mut handle, text, "output");
}

/// Print the version banner (`van <crate-version>`) to stdout.
fn print_version() -> Result<(), Error> {
    let line: String = version_line();
    return print_line(&line);
}
