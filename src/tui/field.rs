//! Single-line text field widget.
//!
//! The field stores one editable value as characters, edits it at the
//! cursor, and draws one inline `[....]` slot. It does not manage focus,
//! compose fields into a form, or place the terminal cursor: those
//! belong to the caller.

use std::io;
use std::io::Write;

use crate::common::Error;
use crate::tui::terminal::LineWriter;

/// The character drawn in the unused positions of a slot.
const PADDING: char = '.';

/// A single-line editable value drawn as one inline slot.
pub struct TextField {
    /// The stored characters, in display order.
    ///
    /// Characters are stored instead of a `String` so that the cursor is
    /// an index into characters. A byte index would split a multi-byte
    /// character, and removing one with `String::remove` would need byte
    /// arithmetic that this representation never has to get right.
    chars: Vec<char>,
    /// Number of stored characters before the edit position, between
    /// `0` and the number of stored characters.
    cursor: usize,
    /// Fixed slot width, which is also the maximum number of characters
    /// the field stores.
    width: usize,
}

impl TextField {
    /// Build an empty field whose slot is `width` characters wide.
    ///
    /// `width` is fixed for the lifetime of the field: it is the width
    /// of the rendered slot and the maximum number of characters the
    /// field accepts.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when `width` is zero, because a slot
    /// without room for a character could not show what was typed into
    /// it.
    pub fn new(width: usize) -> Result<TextField, Error> {
        return match width {
            0 => Err(Error::Runtime(String::from(
                "text field width must be greater than zero",
            ))),
            _ => Ok(TextField {
                chars: Vec::new(),
                cursor: 0,
                width,
            }),
        };
    }

    /// Return the stored characters as a `String`.
    pub fn value(&self) -> String {
        let mut value = String::new();
        for character in &self.chars {
            value.push(*character);
        }
        return value;
    }

    /// Return true when the field stores no characters.
    pub fn is_empty(&self) -> bool {
        return self.chars.is_empty();
    }

    /// Insert `character` at the cursor and advance the cursor.
    ///
    /// A field that already holds `width` characters ignores the call:
    /// the value and the cursor stay as they are, because the slot has
    /// no room to draw another character. Typing into a full field is a
    /// silent no-op rather than an error, so the interactive session
    /// keeps running.
    pub fn insert_char(&mut self, character: char) {
        if self.chars.len() >= self.width {
            return;
        }
        self.chars.insert(self.cursor, character);
        self.cursor += 1;
    }

    /// Remove the character before the cursor.
    ///
    /// The cursor moves back with the character. At the start of the
    /// value the call changes nothing.
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let position = self.cursor - 1;
        self.chars.remove(position);
        self.cursor = position;
    }

    /// Remove the character at the cursor.
    ///
    /// The cursor stays where it is, so the character that followed the
    /// removed one is edited next. At the end of the value the call
    /// changes nothing.
    pub fn delete(&mut self) {
        if self.cursor >= self.chars.len() {
            return;
        }
        self.chars.remove(self.cursor);
    }

    /// Move the cursor one character left, or stay at the start.
    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
    }

    /// Move the cursor one character right, or stay at the end.
    pub fn move_right(&mut self) {
        if self.cursor >= self.chars.len() {
            return;
        }
        self.cursor += 1;
    }

    /// Return the terminal cursor offset inside the rendered slot.
    ///
    /// The offset counts the opening bracket as `0`, so `1` places the
    /// cursor directly after `[`, which is where a later task positions
    /// the hardware cursor. The offset is always one more than the
    /// number of characters before the edit position.
    pub fn cursor_offset(&self) -> usize {
        return self.cursor + 1;
    }

    /// Write the field's slot: `[`, the value padded with `.`, `]`.
    ///
    /// The content is padded to exactly `width` characters, so the two
    /// brackets of a slot are always `width` characters apart and a row
    /// of slots lines up. Nothing else is written: no label, no newline,
    /// and no ANSI sequence.
    ///
    /// # Errors
    ///
    /// Returns the error reported by the underlying writer.
    pub fn render_slot<W: Write>(
        &self,
        writer: &mut LineWriter<W>,
    ) -> io::Result<()> {
        let mut slot = String::new();
        slot.push('[');
        for character in &self.chars {
            slot.push(*character);
        }
        let mut filled = self.chars.len();
        while filled < self.width {
            slot.push(PADDING);
            filled += 1;
        }
        slot.push(']');
        return writer.write_text(&slot);
    }
}
