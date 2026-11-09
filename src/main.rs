//! CLI entry point for the `van` package and plugin manager.

use std::process::ExitCode;

use vantools::common::VanError;
use vantools::common::cli::to_exit_code;

mod cli;

fn main() -> ExitCode {
    let result = cli::app::run();
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
