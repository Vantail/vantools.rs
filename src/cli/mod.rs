//! Terminal interaction for the `van` binary.
//!
//! Everything that reads process arguments or writes to the terminal
//! lives here: the orchestration in [`app`], the shared flag parsing in
//! [`parser`], the subcommand wrappers in [`pack`], [`unpack`], and
//! [`verify`], and the report formatting in [`output`] and
//! [`verify_report`]. The library modules they call return structured
//! reports and print nothing, and the plain log helpers they share live
//! in [`output`].

pub mod app;
pub mod output;
pub mod pack;
pub mod parser;
pub mod unpack;
pub mod verify;
pub mod verify_report;
