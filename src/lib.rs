//! Command-line toolset for working with `.van` plugin archives.
//!
//! The crate builds a single binary named `van`. It dispatches
//! subcommands in [`van`] and implements the `pack` subcommand in
//! [`pack`]; shared types and helpers live in [`common`].

#![forbid(unsafe_code)]

pub mod common;
pub mod pack;
pub mod van;
