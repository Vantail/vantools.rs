//! Shared types for the `van` CLI: the subcommand blueprint and the
//! unified error type.

pub mod archive;
pub mod cli;
pub mod error;
pub mod fs;
pub mod manifest;
pub mod plugin;
pub mod tree;
pub mod validation;
pub mod walk;

pub use error::{Error, Result, VanError};

/// Blueprint every `van` subcommand must implement.
///
/// The dispatcher in [`crate::van`] only knows this trait; concrete
/// subcommands are added in later phases.
pub trait Subcommand {
    /// Run the subcommand.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if the subcommand fails.
    fn execute(&self) -> Result<()>;
}
