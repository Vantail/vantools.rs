//! Single-line text field widget.
//!
//! The field stores one editable value as characters, edits it at the
//! cursor, and draws one inline `[....]` slot. The slot keeps a fixed
//! width, so a longer value is shown as a window that slides with the
//! cursor. The widget does not manage focus, compose fields into a form,
//! or place the terminal cursor: those belong to the caller.

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
    /// Fixed slot width: the number of character cells the slot draws,
    /// and the size of the visible window into the value.
    width: usize,
    /// Index of the first character shown in the slot.
    ///
    /// After every change the window keeps `offset` at or before the
    /// cursor and the cursor cell inside the slot, so the cursor is
    /// never scrolled out of view.
    offset: usize,
}

impl TextField {
    /// Build an empty field whose slot is `width` characters wide.
    ///
    /// `width` is fixed for the lifetime of the field: it is the width
    /// of the rendered slot and of the visible window into the value.
    /// The value itself has no length limit, so a value longer than the
    /// slot scrolls instead of being rejected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when `width` is zero, because a slot
    /// without room for one character could not show the cursor.
    pub fn new(width: usize) -> Result<TextField, Error> {
        return match width {
            0 => Err(Error::Runtime(String::from(
                "text field width must be greater than zero",
            ))),
            _ => Ok(TextField {
                chars: Vec::new(),
                cursor: 0,
                width,
                offset: 0,
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
    /// The value has no length limit. When it no longer fits the slot,
    /// the window slides just enough to keep the cursor visible, so
    /// typing always shows where the next character goes.
    pub fn insert_char(&mut self, character: char) {
        self.chars.insert(self.cursor, character);
        self.cursor += 1;
        self.update_offset();
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
        self.update_offset();
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
        self.update_offset();
    }

    /// Move the cursor one character left, or stay at the start.
    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        self.update_offset();
    }

    /// Move the cursor one character right, or stay at the end.
    pub fn move_right(&mut self) {
        if self.cursor >= self.chars.len() {
            return;
        }
        self.cursor += 1;
        self.update_offset();
    }

    /// Return the terminal cursor offset inside the rendered slot.
    ///
    /// The offset counts the opening bracket as `0`, so the position lies
    /// within `1..=width`: the window keeps the cursor on a stored
    /// character or on the padding dot after the value. The hardware
    /// cursor is therefore always visible, and the position is relative
    /// to the slot rather than to the value.
    pub fn cursor_offset(&self) -> usize {
        return self.cursor - self.offset + 1;
    }

    /// Write the field's slot: `[`, one window of the value, `]`.
    ///
    /// The window content is character-based: every stored character
    /// occupies exactly one cell, so the slot is exactly `width`
    /// characters wide however many bytes a character needs. The window
    /// is padded with [`PADDING`] when the value ends inside it.
    ///
    /// Nothing else is written: no label, no newline, and no ANSI
    /// sequence.
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
        let mut filled = 0;
        for (position, character) in self.chars.iter().enumerate() {
            if filled >= self.width {
                break;
            }
            if position < self.offset {
                continue;
            }
            slot.push(*character);
            filled += 1;
        }
        while filled < self.width {
            slot.push(PADDING);
            filled += 1;
        }
        slot.push(']');
        return writer.write_text(&slot);
    }

    /// Slide the window so the cursor stays visible inside the slot.
    ///
    /// The rules run in order and the first one that matches decides the
    /// window, so the window moves only as far as the cursor requires.
    fn update_offset(&mut self) {
        // A value shorter than the slot fits in it with room to spare,
        // so the whole value is visible and there is nothing to scroll.
        if self.chars.len() < self.width {
            self.offset = 0;
            return;
        }
        // The cursor would sit past the right edge. Moving the window
        // until the cursor occupies the rightmost cell leaves the
        // character before the cursor in view; at the end of the value
        // that cell holds a padding dot, which marks where typing goes.
        if self.offset + self.width <= self.cursor {
            self.offset = self.cursor - self.width + 1;
            return;
        }
        // The cursor would sit left of the window, so it becomes the
        // first visible character.
        if self.cursor < self.offset {
            self.offset = self.cursor;
            return;
        }
    }
}
