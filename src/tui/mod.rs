//! Terminal user interface helpers.
//!
//! The module collects the presentation code the `van` binary shares:
//! [`log`] writes the tagged log lines, [`logo`] prints the ASCII
//! wordmark, [`progress`] draws the cosmetic block bar the subcommands
//! share before their real work starts, [`tree`] renders a flat
//! listing of entry names as an ASCII tree, [`select`] shows a generic
//! interactive selection menu, and [`terminal`] guards the terminal
//! state such a menu needs.

pub mod log;
pub mod logo;
pub mod progress;
pub mod select;
pub mod terminal;
pub mod tree;
