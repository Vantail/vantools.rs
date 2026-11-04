//! CLI entry point for the `van` package and plugin manager.

use std::process::ExitCode;

use vantools::common::VanError;
use vantools::van;

/// Exit code used when an error reports a value outside the `u8` range.
const FALLBACK_EXIT_CODE: u8 = 1;

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
    return ExitCode::from(to_exit_code(code));
}

/// Convert an error's exit code to a valid process exit code.
///
/// Values outside the `0..=255` range are mapped to a generic failure
/// code so a failed run is never reported as successful.
fn to_exit_code(code: i32) -> u8 {
    return match u8::try_from(code) {
        Ok(value) => value,
        Err(_) => FALLBACK_EXIT_CODE,
    };
}

#[cfg(test)]
mod tests {
    use super::{FALLBACK_EXIT_CODE, to_exit_code};

    #[test]
    fn in_range_exit_codes_are_preserved() {
        assert_eq!(to_exit_code(0), 0);
        assert_eq!(to_exit_code(1), 1);
        assert_eq!(to_exit_code(255), 255);
    }

    #[test]
    fn out_of_range_exit_codes_map_to_failure() {
        assert_eq!(to_exit_code(-1), FALLBACK_EXIT_CODE);
        assert_eq!(to_exit_code(256), FALLBACK_EXIT_CODE);
        assert_eq!(to_exit_code(i32::MAX), FALLBACK_EXIT_CODE);
    }
}
