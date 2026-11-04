//! CLI entry point for the `van` package and plugin manager.

use std::io;
use std::process::ExitCode;

use vantools::cli::{report_error, to_exit_code};
use vantools::common::VanError;
use vantools::van;

fn main() -> ExitCode {
    let result = van::run();
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
