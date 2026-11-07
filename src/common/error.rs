//! Unified error handling for the `van` CLI.

use std::fmt;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Blueprint for all `van` errors.
///
/// Every error exposes a stable process exit code so the CLI
/// terminates with a predictable status.
pub trait VanError: std::error::Error {
    /// Process exit code associated with this error.
    fn exit_code(&self) -> i32;
}

/// All errors the `van` CLI can produce in Phase 0.1.
///
/// The enum is `non_exhaustive` so later phases can add variants
/// without a breaking change, and because `clippy::exhaustive_enums`
/// is denied for this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The user invoked the CLI incorrectly.
    Usage(String),
    /// A runtime error that is not a usage error.
    Runtime(String),
    /// An archive path or file type failed validation.
    ValidationError(String),
    /// The arguments of a subcommand are invalid.
    CliError(String),
}

impl VanError for Error {
    fn exit_code(&self) -> i32 {
        return match self {
            Error::Usage(_) => 2,
            Error::Runtime(_)
            | Error::ValidationError(_)
            | Error::CliError(_) => 1,
        };
    }
}

impl fmt::Display for Error {
    /// Render the error with the `error: ` prefix shared by every variant.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Error::Usage(message)
            | Error::Runtime(message)
            | Error::ValidationError(message)
            | Error::CliError(message) => message,
        };
        return write!(formatter, "error: {message}");
    }
}

impl std::error::Error for Error {}
