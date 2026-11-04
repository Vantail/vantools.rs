//! Defines the interface for CLI commands.

use crate::common::Error;

/// A CLI command that can be executed.
pub trait Command {
    /// Executes the command.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if the command fails.
    fn run(&self) -> Result<(), Error>;
}
