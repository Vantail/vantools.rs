//! Terminal user interface helpers.
//!
//! The module collects the presentation code the `van` binary shares:
//! [`log`] writes the tagged log lines, [`progress`] draws the cosmetic
//! block bar the subcommands share before their real work starts,
//! [`tree`] renders a flat listing of entry names as an ASCII tree,
//! [`menu`] draws the interactive start menu shown by an argument-less
//! run, and [`form`] collects the `pack` parameters when that command is
//! chosen from the menu.

pub mod form;
pub mod log;
pub mod menu;
pub mod progress;
pub mod tree;
