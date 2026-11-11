//! Form widget: one command line built from text segments and fields.
//!
//! The form is a fixed composition. Rendered, it alternates a text
//! segment and a field slot and ends with the final segment, so the whole
//! form is exactly one line such as
//! `van pack -s [....] -t [....]`. Tab and BackTab cycle the focus, the
//! editing keys act on the focused field, and [`Form::cursor_column`]
//! reports where the caller places the hardware cursor. The widget emits
//! no ANSI sequences, validates no field contents, and executes nothing.

use std::io;
use std::io::Write;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::common::Error;
use crate::tui::field::TextField;
use crate::tui::terminal::{InputEvent, LineWriter};

/// The result of handling one input event.
///
/// The enum is `non_exhaustive` because `clippy::exhaustive_enums` is
/// denied for this crate.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormOutcome {
    /// The event changed nothing the widget draws.
    Nothing,
    /// The focus, the values, or the cursor moved, so the line must be
    /// drawn again.
    Redraw,
    /// The user asked to run the command the form describes.
    Submitted,
    /// The user asked to abandon the form.
    Cancelled,
}

/// A command line composed of text segments and editable fields.
pub struct Form {
    /// The text segments; the form holds one more segment than fields,
    /// because a field sits between every pair of segments.
    segments: Vec<String>,
    /// Character length of each segment, in the same order.
    segment_lengths: Vec<usize>,
    /// The editable fields, in display order.
    fields: Vec<TextField>,
    /// Slot width of each field, in the same order.
    widths: Vec<usize>,
    /// Index of the focused field.
    focus: usize,
}

impl Form {
    /// Build a form from its text segments and the widths of its fields.
    ///
    /// The composition contract is that `segments` holds exactly one more
    /// text than `widths` holds fields. Rendering then writes segment
    /// `i`, the slot of field `i`, and finally the last segment. The
    /// first field starts focused.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when `widths` is empty, when the two
    /// counts do not compose one line, or when a segment contains a line
    /// break, and returns the [`TextField::new`] error unchanged when a
    /// width is zero.
    pub fn new(segments: &[String], widths: &[usize]) -> Result<Form, Error> {
        let counts_match = segments.len() == widths.len() + 1;
        let mut broken = false;
        for segment in segments {
            if has_line_break(segment) {
                broken = true;
            }
        }
        // A segment is written as one piece of the single rendered line,
        // so a line break inside it would end that line early and split
        // the command across two rows.
        let rejection = match widths.is_empty() {
            true => Some(String::from("form requires at least one field")),
            false => match counts_match {
                true => match broken {
                    true => Some(String::from(
                        "form text must not contain line breaks",
                    )),
                    false => None,
                },
                false => Some(String::from(
                    "form requires exactly one more text segment than fields",
                )),
            },
        };
        return match rejection {
            Some(message) => Err(Error::Runtime(message)),
            None => build(segments, widths),
        };
    }

    /// Return the index of the focused field.
    pub fn focus(&self) -> usize {
        return self.focus;
    }

    /// Apply one input event and report what the caller should do next.
    ///
    /// A cancellation is reported to the caller, which routes it back to
    /// the main menu; the widget itself knows nothing about the menu.
    pub fn handle(&mut self, event: InputEvent) -> FormOutcome {
        return match event {
            InputEvent::Quit => FormOutcome::Cancelled,
            InputEvent::Key {
                code,
                modifiers,
            } => self.handle_key(code, modifiers),
            InputEvent::Resize {
                ..
            } => FormOutcome::Nothing,
        };
    }

    /// Write the form's single line through `writer`.
    ///
    /// The output alternates each text segment with the slot of the field
    /// that follows it and ends with the last segment, terminated by one
    /// `\r\n`. It therefore contains exactly one line, with no labels, no
    /// added spaces, and no ANSI sequence. A trailing empty segment adds
    /// nothing, so the line has no trailing spaces.
    ///
    /// # Errors
    ///
    /// Returns the error reported by the underlying writer.
    pub fn render<W: Write>(
        &self,
        writer: &mut LineWriter<W>,
    ) -> io::Result<()> {
        for (index, field) in self.fields.iter().enumerate() {
            let segment = match self.segments.get(index) {
                Some(value) => value,
                None => return Ok(()),
            };
            let written_segment = writer.write_text(segment);
            match written_segment {
                Ok(()) => {}
                Err(error) => return Err(error),
            }
            let written_slot = field.render_slot(writer);
            match written_slot {
                Ok(()) => {}
                Err(error) => return Err(error),
            }
        }
        let last = match self.segments.get(self.fields.len()) {
            Some(value) => value,
            None => return Ok(()),
        };
        let written_last = writer.write_text(last);
        match written_last {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        return writer.write_line("");
    }

    /// Return the on-screen column of the hardware cursor.
    ///
    /// The column counts from `0` at the start of the rendered line: the
    /// segments up to and including the focused one, the slot of every
    /// field before the focused one, and finally the cursor offset inside
    /// the focused slot. The caller passes this value to the terminal
    /// cursor positioning after drawing the form.
    pub fn cursor_column(&self) -> usize {
        let mut column = 0;
        let mut index = 0;
        while index <= self.focus {
            let length = match self.segment_lengths.get(index) {
                Some(value) => *value,
                None => 0,
            };
            column += length;
            if index < self.focus {
                let width = match self.widths.get(index) {
                    Some(value) => *value,
                    None => 0,
                };
                column += width + 2;
            }
            index += 1;
        }
        let focused = match self.fields.get(self.focus) {
            Some(value) => value,
            None => return column,
        };
        return column + focused.cursor_offset();
    }

    /// Return the value of every field, in field order.
    pub fn values(&self) -> Vec<String> {
        let mut collected: Vec<String> = Vec::new();
        for field in &self.fields {
            collected.push(field.value());
        }
        return collected;
    }

    /// Apply one key press and report what it requires.
    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> FormOutcome {
        return match code {
            KeyCode::Tab => {
                self.focus_next();
                FormOutcome::Redraw
            }
            // Terminals deliver Shift+Tab as `BackTab` rather than as
            // `Tab` with a modifier, so the two directions need separate
            // arms.
            KeyCode::BackTab => {
                self.focus_previous();
                FormOutcome::Redraw
            }
            KeyCode::Char(character) => {
                self.handle_character(character, modifiers)
            }
            KeyCode::Left => {
                self.edit(TextField::move_left);
                FormOutcome::Redraw
            }
            KeyCode::Right => {
                self.edit(TextField::move_right);
                FormOutcome::Redraw
            }
            KeyCode::Backspace => {
                self.edit(TextField::backspace);
                FormOutcome::Redraw
            }
            KeyCode::Delete => {
                self.edit(TextField::delete);
                FormOutcome::Redraw
            }
            // Submitting with empty fields is allowed: what a value must
            // look like, and what a submission runs, is the caller's
            // decision.
            KeyCode::Enter => FormOutcome::Submitted,
            // `crossterm::event::KeyCode` is `#[non_exhaustive]`, so this
            // arm is mandatory; every other key changes nothing.
            _ => FormOutcome::Nothing,
        };
    }

    /// Insert a typed character into the focused field.
    ///
    /// The character of a `Control`, `Alt`, or `Super` combination is a
    /// shortcut rather than text, so it is not inserted. `Shift` stays
    /// allowed, because a terminal reports an uppercase letter as the
    /// shifted character and that is ordinary input.
    fn handle_character(
        &mut self,
        character: char,
        modifiers: KeyModifiers,
    ) -> FormOutcome {
        let shortcuts =
            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER;
        let shortcut = modifiers.intersects(shortcuts);
        return match shortcut {
            true => FormOutcome::Nothing,
            false => {
                self.edit(|field| field.insert_char(character));
                FormOutcome::Redraw
            }
        };
    }

    /// Apply one editing operation to the focused field.
    fn edit<F: FnOnce(&mut TextField)>(&mut self, operation: F) {
        let field = self.fields.get_mut(self.focus);
        if let Some(value) = field {
            operation(value);
        }
    }

    /// Advance the focus to the next field, wrapping after the last.
    fn focus_next(&mut self) {
        let last = self.fields.len() - 1;
        if self.focus < last {
            self.focus += 1;
            return;
        }
        self.focus = 0;
    }

    /// Move the focus to the previous field, wrapping before the first.
    fn focus_previous(&mut self) {
        if self.focus > 0 {
            self.focus -= 1;
            return;
        }
        self.focus = self.fields.len() - 1;
    }
}

/// Build the fields and cached segment lengths of a validated form.
///
/// The widths are known to be non-zero and the counts are known to
/// compose one line, so the only error this can report is the one
/// [`TextField::new`] returns for a width it rejects.
fn build(segments: &[String], widths: &[usize]) -> Result<Form, Error> {
    let mut fields: Vec<TextField> = Vec::new();
    let mut copied_widths: Vec<usize> = Vec::new();
    for width in widths {
        // The width error is returned unchanged, and `clippy` rejects a
        // `match` that only re-returns it, so `?` is used here.
        let field = TextField::new(*width)?;
        fields.push(field);
        copied_widths.push(*width);
    }
    let mut copied_segments: Vec<String> = Vec::new();
    let mut lengths: Vec<usize> = Vec::new();
    for segment in segments {
        let mut length = 0;
        for _character in segment.chars() {
            length += 1;
        }
        lengths.push(length);
        copied_segments.push(String::from(segment.as_str()));
    }
    return Ok(Form {
        segments: copied_segments,
        segment_lengths: lengths,
        fields,
        widths: copied_widths,
        focus: 0,
    });
}

/// Returns true when `text` contains a line break.
fn has_line_break(text: &str) -> bool {
    return text.contains('\n') || text.contains('\r');
}
