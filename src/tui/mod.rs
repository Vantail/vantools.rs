//! Terminal user interface helpers.
//!
//! The module collects the presentation code the `van` binary shares:
//! [`log`] writes the tagged log lines, [`progress`] draws the cosmetic
//! block bar the subcommands share before their real work starts,
//! [`tree`] renders a flat listing of entry names as an ASCII tree, and
//! [`menu`] draws the interactive start menu shown by an argument-less
//! run.

pub mod log;
pub mod menu;
pub mod progress;
pub mod tree;
