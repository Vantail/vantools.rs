//! Core library for working with `.van` plugin archives.
//!
//! The crate exposes the data-processing parts of the tool: shared types
//! and helpers in [`common`], the `pack` command in [`pack`], the
//! `unpack` command in [`unpack`], the `verify` command in [`verify`],
//! and the subcommand dispatcher in [`van`]. The commands return
//! structured reports and print nothing; the terminal helpers in [`tui`]
//! render those reports for the `van` binary.

#![forbid(unsafe_code)]

pub mod common;
pub mod pack;
pub mod tui;
pub mod unpack;
pub mod van;
pub mod verify;
