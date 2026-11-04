//! CLI entry point for the `van` package and plugin manager.

use std::process::ExitCode;

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
    eprintln!("{error}");
    let code: i32 = error.exit_code();
    return ExitCode::from(code as u8);
}
