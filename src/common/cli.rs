//! CLI support helpers for collecting arguments and mapping exit codes.

use std::env;

use crate::common::Error;

/// Exit code used when an error reports a value outside the `u8` range.
pub const FALLBACK_EXIT_CODE: u8 = 1;

/// Convert an error's exit code to a valid process exit code.
///
/// Values outside the `0..=255` range are mapped to a generic failure
/// code so a failed run is never reported as successful.
pub fn to_exit_code(code: i32) -> u8 {
    return match u8::try_from(code) {
        Ok(value) => value,
        Err(_) => FALLBACK_EXIT_CODE,
    };
}

/// Collect the process arguments after the program name.
///
/// Arguments that are not valid Unicode are rejected as usage errors
/// rather than aborting the process.
///
/// # Errors
///
/// Returns [`Error::Usage`] when an argument is not valid Unicode.
pub fn collect_arguments() -> Result<Vec<String>, Error> {
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

/// Render the version banner (`van <crate-version>`).
pub fn version_line() -> String {
    let version: &str = env!("CARGO_PKG_VERSION");
    return format!("van {version}");
}

/// Metadata and behavior shared by every CLI command.
///
/// Implementors describe themselves so the entry point can print help
/// and usage text without duplicating the wording.
pub trait CliCommand {
    /// The name of the command, for example `pack`.
    fn name(&self) -> &'static str;

    /// The usage line of the command, without a trailing newline.
    fn usage(&self) -> &'static str;

    /// The full help text of the command, without a trailing newline.
    fn help(&self) -> &'static str;
}
