//! Command-line toolset for working with `.van` plugin archives.
//!
//! The crate builds a single binary named `van`. It dispatches
//! subcommands in [`van`], implements the `pack` subcommand in [`pack`],
//! and implements the `verify` subcommand in [`verify`]; shared types
//! and helpers live in [`common`], and the terminal-only progress
//! animation lives in [`tui`].

#![forbid(unsafe_code)]

pub mod common;
pub mod pack;
pub mod tui;
pub mod van;
pub mod verify;
