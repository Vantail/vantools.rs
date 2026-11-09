//! Terminal user interface helpers.
//!
//! The module collects the presentation code the `van` binary shares:
//! [`log`] writes the tagged log lines, [`progress`] draws the cosmetic
//! block bar the subcommands share before their real work starts, and
//! [`tree`] renders a flat listing of entry names as an ASCII tree.

pub mod log;
pub mod progress;
pub mod tree;
