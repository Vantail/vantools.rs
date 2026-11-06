//! Terminal user interface helpers for the `van` binary.
//!
//! The module collects presentation code: [`logo`] draws the startup
//! banner, [`log`] writes the tagged log lines, [`progress`] draws the
//! cosmetic block bar the subcommands share before their real work
//! starts, and [`tree`] renders a flat listing of entry names as an
//! ASCII tree.

pub mod log;
pub mod logo;
pub mod progress;
pub mod tree;
