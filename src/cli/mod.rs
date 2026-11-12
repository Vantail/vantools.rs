//! Terminal interaction for the `van` binary.
//!
//! Everything that reads process arguments or writes to the terminal
//! lives here: the orchestration in [`app`], the command table in
//! [`commands`], the shared flag parsing in [`parser`], the subcommand
//! wrappers in [`pack`], [`unpack`], and [`verify`], and the report
//! formatting in [`output`] and [`verify_report`]. The library modules
//! they call return structured reports and print nothing, and the plain
//! log helpers they share live in [`output`].

pub mod app;
/// The command table of the interactive interface: one entry per command
/// with its menu label, form, progress messages, and runner.
mod commands;
/// Interactive terminal mode: the menu, its parameter forms, and the
/// command runs they start.
mod interactive;
pub mod output;
pub mod pack;
pub mod parser;
pub mod unpack;
pub mod verify;
pub mod verify_report;
