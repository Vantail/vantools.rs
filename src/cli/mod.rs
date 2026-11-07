//! Terminal interaction for the `van` binary.
//!
//! Everything that reads process arguments or writes to the terminal
//! lives here: the shared flag parsing in [`parser`], the subcommand
//! wrappers in [`pack`], [`unpack`], and [`verify`], the report
//! formatting in [`output`] and [`verify_report`], and the cosmetic
//! [`tui`] helpers. The library modules they call return structured
//! reports and print nothing.

pub mod output;
pub mod pack;
pub mod parser;
pub mod tui;
pub mod unpack;
pub mod verify;
pub mod verify_report;
