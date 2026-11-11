//! Terminal foundation for the interactive interface.
//!
//! The module owns the pieces that touch the terminal directly: the
//! carriage-return-safe line writer, the raw-mode and alternate-screen
//! guard, and the input layer. Drawing and menu logic are added in later
//! tasks.

pub mod terminal;
