//! Terminal foundation for the interactive interface.
//!
//! The module owns the pieces that touch the terminal directly: the
//! carriage-return-safe line writer, the raw-mode and alternate-screen
//! guard, the input layer, the menu widget that holds selection state
//! and draws one frame, and the text field widget that stores and draws
//! one editable value. Drawing and session wiring are added in later
//! tasks.

pub mod field;
pub mod menu;
pub mod terminal;
