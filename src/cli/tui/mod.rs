//! Terminal user interface helpers for the `van` binary.
//!
//! The module collects presentation code: [`progress`] draws the
//! cosmetic animation the subcommands share before their real work
//! starts, and [`tree`] renders a flat listing of entry names as an
//! ASCII tree.

pub mod progress;
pub mod tree;
