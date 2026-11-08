//! Interactive start menu for the `van` binary.
//!
//! The menu is shown when `van` runs without arguments. It draws the
//! wordmark, the slogan, and the four top-level commands, and returns
//! the command the user confirmed. Choosing a command keeps that layout
//! on screen and draws its parameter form below the command list, so the
//! logo and the menu stay visible while the paths are typed. Raw mode
//! and the alternate screen belong to a [`TerminalGuard`], which
//! restores both when the menu leaves its scope, including when it
//! returns an error.
//!
//! The layout is written for at least 80 columns. A narrower window
//! gets a short notice and exits on the next key press, because a
//! wrapped frame breaks the wordmark and scatters the menu rows.

use std::io;
use std::io::IsTerminal;
use std::io::Write;

use crate::common::Error;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::queue;
use crossterm::style::{Print, Stylize};
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
    disable_raw_mode, enable_raw_mode, size,
};

/// The start menu wordmark, one entry per row.
///
/// The rows are kept verbatim: the face depends on the exact column of
/// every character, so no row may be reflowed or trimmed.
pub const LOGO_ROWS: [&str; 8] = [
    "                  _______          _     ",
    "                 |__   __|        | |    ",
    " __   ____ _ _ __   | | ___   ___ | |___ ",
    r" \ \ / / _` | '_ \  | |/ _ \ / _ \| / __|",
    r"  \ V / (_| | | | |_| | (_) | (_) | \__ \",
    r"   \_/ \__,_|_| |_(_)_|\___/ \___/|_|___/",
    "                                         ",
    "                                         ",
];

/// ANSI SGR sequence that switches to italics.
const ITALIC: &str = "\x1b[3m";

/// ANSI SGR sequence that resets every attribute.
const RESET: &str = "\x1b[0m";

/// ANSI SGR sequence that marks the active form field.
const ACTIVE_FIELD: &str = "\x1b[1;32m";

/// Rainbow palette the logo cycles through, left to right.
const RAINBOW: [&str; 6] = [
    "\x1b[31m", // Red
    "\x1b[33m", // Yellow
    "\x1b[32m", // Green
    "\x1b[36m", // Cyan
    "\x1b[34m", // Blue
    "\x1b[35m", // Magenta
];

/// Slogan shown below the wordmark.
pub const SLOGAN: &str = "The vantail package and plugin manager CLI";

/// Hint line shown below the commands.
const HINT: &str = "  ↑↓ Select | Enter Confirm | Q Quit";

/// Minimum number of columns the start menu is laid out for.
pub const MIN_WIDTH: u16 = 80;

/// Message shown when the window is narrower than 80 columns.
const NARROW_MESSAGE: &str =
    "Terminal is too narrow. Please resize your window to at least 80 columns.";

/// Prompt shown below the narrow message.
const NARROW_HINT: &str = "Press any key to exit.";

/// Width the command names are padded to before the description.
const LABEL_WIDTH: usize = 10;

/// Character the separator above the form is drawn with.
const SEPARATOR_CHARACTER: char = '-';

/// Width of the separator above the form.
const SEPARATOR_WIDTH: usize = 50;

/// Text before the source value in the form line.
const FORM_SOURCE: &str = " --source ";

/// Text before the target value in the form line.
const FORM_TARGET: &str = " --target ";

/// Text between the source value and the target value in the form line.
const FIELD_GAP: &str = "] --target [";

/// Hint shown under a form with two fields.
const TWO_FIELD_HINT: &str = "(Tab to switch, Enter to confirm, Esc to cancel)";

/// Hint shown under a form with one field.
const SINGLE_FIELD_HINT: &str = "(Enter to confirm, Esc to cancel)";

/// Index of the source field in a form.
const SOURCE_FIELD: usize = 0;

/// Index of the target field in a form.
const TARGET_FIELD: usize = 1;

/// Command the user picked in the start menu.
#[derive(Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MenuChoice {
    /// Compress a plugin directory into an archive.
    Pack,
    /// Validate an archive before it is used.
    Verify,
    /// Extract an archive into a plugin directory.
    Unpack,
    /// Leave the application.
    Quit,
}

/// One selectable row of the start menu.
struct MenuEntry {
    /// Command returned when the row is confirmed.
    choice: MenuChoice,
    /// Command name shown in the row.
    label: &'static str,
    /// One-line description shown after the name.
    description: &'static str,
}

/// Rows of the start menu, in display order.
const MENU_ENTRIES: [MenuEntry; 4] = [
    MenuEntry {
        choice: MenuChoice::Pack,
        label: "Pack",
        description: "Compress a plugin directory",
    },
    MenuEntry {
        choice: MenuChoice::Verify,
        label: "Verify",
        description: "Validate an archive",
    },
    MenuEntry {
        choice: MenuChoice::Unpack,
        label: "Unpack",
        description: "Extract an archive",
    },
    MenuEntry {
        choice: MenuChoice::Quit,
        label: "Quit",
        description: "Exit the application",
    },
];

/// Screen the start menu currently shows.
///
/// The command list and the parameter form share one state value, so a
/// frame can be rebuilt for either screen without clearing the parts
/// that stay visible.
#[non_exhaustive]
pub enum AppState {
    /// The command list, with the index of the highlighted row.
    Menu(usize),
    /// The parameter form of the chosen command.
    Form {
        /// Command whose parameters are being entered.
        command: MenuChoice,
        /// Index of the field that receives typed characters.
        active_field: usize,
        /// Characters typed into the source field.
        source_buffer: String,
        /// Characters typed into the target field.
        target_buffer: String,
    },
}

impl AppState {
    /// Create the command-list state with `selected` highlighted.
    pub fn menu(selected: usize) -> Self {
        return AppState::Menu(selected);
    }

    /// Create an empty `pack` form.
    pub fn pack_form() -> Self {
        return AppState::form(MenuChoice::Pack);
    }

    /// Create an empty `unpack` form.
    pub fn unpack_form() -> Self {
        return AppState::form(MenuChoice::Unpack);
    }

    /// Create an empty `verify` form.
    pub fn verify_form() -> Self {
        return AppState::form(MenuChoice::Verify);
    }

    /// Create an empty form for `command`.
    fn form(command: MenuChoice) -> Self {
        return AppState::Form {
            command,
            active_field: SOURCE_FIELD,
            source_buffer: String::new(),
            target_buffer: String::new(),
        };
    }
}

/// What the start menu finished with.
///
/// A command chosen from the list carries the paths typed into its form;
/// quitting carries none. The binary only reads the value, so the fields
/// stay private.
pub struct MenuSelection {
    choice: MenuChoice,
    source: Option<String>,
    target: Option<String>,
}

impl MenuSelection {
    /// Return the command the user chose.
    pub fn choice(&self) -> MenuChoice {
        return self.choice;
    }

    /// Return the source path entered in the form, when one was asked for.
    pub fn source(&self) -> Option<&str> {
        return match &self.source {
            Some(value) => Some(value.as_str()),
            None => None,
        };
    }

    /// Return the target path entered in the form, when one was asked for.
    pub fn target(&self) -> Option<&str> {
        return match &self.target {
            Some(value) => Some(value.as_str()),
            None => None,
        };
    }

    /// Build the selection of a form submission.
    fn from_form(command: MenuChoice, source: &str, target: &str) -> Self {
        let mut target_value: Option<String> = None;
        if field_count(command) >= 2 {
            target_value = Some(String::from(target));
        }
        return MenuSelection {
            choice: command,
            source: Some(String::from(source)),
            target: target_value,
        };
    }

    /// Build the selection of a cancelled menu.
    fn quit() -> Self {
        return MenuSelection {
            choice: MenuChoice::Quit,
            source: None,
            target: None,
        };
    }
}

/// Show the start menu and return what the user finished with.
///
/// The terminal must be interactive: raw mode cannot be enabled for a
/// pipe, and a redirected stdin would never deliver a key press.
///
/// Choosing a command opens its form below the unchanged layout. Esc in
/// the form returns to the command list, Enter submits a complete form,
/// and the returned selection then carries the typed paths.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when there is no interactive terminal,
/// when raw mode or the alternate screen cannot be enabled, or when
/// the terminal cannot be drawn or read.
pub fn show_menu() -> Result<MenuSelection, Error> {
    if !is_interactive() {
        return Err(Error::Runtime(String::from(
            "the start menu requires an interactive terminal",
        )));
    }
    let _guard = TerminalGuard::acquire()?;
    if terminal_width() < MIN_WIDTH {
        return show_narrow_notice();
    }
    let mut state = AppState::menu(0);
    loop {
        draw_screen(&state)?;
        let event = match event::read() {
            Ok(value) => value,
            Err(error) => {
                let message = format!("failed to read a key press: {error}");
                return Err(Error::Runtime(message));
            }
        };
        let key = match event {
            Event::Key(key) => key,
            _ => continue,
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let finished = handle_key(&mut state, &key);
        let selection = match finished {
            Some(value) => value,
            None => continue,
        };
        return Ok(selection);
    }
}

/// Apply one key press to the menu state machine.
///
/// Returns a selection when the machine is finished: the chosen command
/// with its form values, or quitting. `None` keeps the menu running.
pub fn handle_key(
    state: &mut AppState,
    key: &KeyEvent,
) -> Option<MenuSelection> {
    if is_form(state) {
        return handle_form_key(state, key);
    }
    return handle_menu_key(state, key);
}

/// Apply one key press to the command list.
fn handle_menu_key(
    state: &mut AppState,
    key: &KeyEvent,
) -> Option<MenuSelection> {
    return match key.code {
        KeyCode::Up => {
            let selected = previous_index(selected_index(state));
            *state = AppState::menu(selected);
            None
        }
        KeyCode::Down => {
            let selected = next_index(selected_index(state));
            *state = AppState::menu(selected);
            None
        }
        KeyCode::Enter => {
            let choice = choice_at(selected_index(state));
            if choice == MenuChoice::Quit {
                return Some(MenuSelection::quit());
            }
            *state = AppState::form(choice);
            None
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(MenuSelection::quit())
        }
        KeyCode::Char('q') => Some(MenuSelection::quit()),
        KeyCode::Esc => Some(MenuSelection::quit()),
        _ => None,
    };
}

/// Apply one key press to the parameter form.
fn handle_form_key(
    state: &mut AppState,
    key: &KeyEvent,
) -> Option<MenuSelection> {
    return match key.code {
        KeyCode::Esc => {
            let selected = selected_index(state);
            *state = AppState::menu(selected);
            None
        }
        KeyCode::Tab => {
            toggle_field(state);
            None
        }
        KeyCode::Enter => submit_form(state),
        KeyCode::Backspace => {
            pop_char(state);
            None
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(MenuSelection::quit())
        }
        KeyCode::Char(character) => {
            if character.is_control() {
                return None;
            }
            push_char(state, character);
            None
        }
        _ => None,
    };
}

/// Append `character` to the active form field.
fn push_char(state: &mut AppState, character: char) {
    let (active_field, source, target) = match state {
        AppState::Form {
            active_field,
            source_buffer,
            target_buffer,
            ..
        } => (active_field, source_buffer, target_buffer),
        AppState::Menu(_) => return,
    };
    if *active_field == SOURCE_FIELD {
        source.push(character);
        return;
    }
    target.push(character);
    return;
}

/// Remove the last character of the active form field, if any.
fn pop_char(state: &mut AppState) {
    let (active_field, source, target) = match state {
        AppState::Form {
            active_field,
            source_buffer,
            target_buffer,
            ..
        } => (active_field, source_buffer, target_buffer),
        AppState::Menu(_) => return,
    };
    if *active_field == SOURCE_FIELD {
        let _removed = source.pop();
        return;
    }
    let _removed = target.pop();
    return;
}

/// Switch to the other field when the command has two.
fn toggle_field(state: &mut AppState) {
    let (command, active_field) = match state {
        AppState::Form {
            command,
            active_field,
            ..
        } => (command, active_field),
        AppState::Menu(_) => return,
    };
    if field_count(*command) < 2 {
        return;
    }
    if *active_field == SOURCE_FIELD {
        *active_field = TARGET_FIELD;
        return;
    }
    *active_field = SOURCE_FIELD;
    return;
}

/// Return the submission of a complete form, if it is complete.
fn submit_form(state: &AppState) -> Option<MenuSelection> {
    let (command, source, target) = match state {
        AppState::Form {
            command,
            source_buffer,
            target_buffer,
            ..
        } => (command, source_buffer, target_buffer),
        AppState::Menu(_) => return None,
    };
    if source.is_empty() {
        return None;
    }
    if field_count(*command) < 2 {
        return Some(MenuSelection::from_form(*command, source, ""));
    }
    if target.is_empty() {
        return None;
    }
    return Some(MenuSelection::from_form(*command, source, target));
}

/// Clear the terminal once the menu has closed.
///
/// The guard has already restored the terminal when this runs, so the
/// command output starts on an empty screen. The clear is best effort:
/// the command's output matters more than the screen.
pub fn clear_screen() {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, Clear(ClearType::All), MoveTo(0, 0));
    return;
}

/// Return the current terminal width in columns.
///
/// A measurement that the platform cannot provide falls back to the
/// minimum width, so the menu still draws instead of failing.
fn terminal_width() -> u16 {
    let measured = size();
    return match measured {
        Ok(value) => value.0,
        Err(_) => MIN_WIDTH,
    };
}

/// Explain that the window is too narrow and wait for one key press.
fn show_narrow_notice() -> Result<MenuSelection, Error> {
    let mut text = String::from(NARROW_MESSAGE);
    text.push_str("\r\n\r\n");
    text.push_str(NARROW_HINT);
    text.push_str("\r\n");
    draw_text(&text)?;
    let read = event::read();
    return match read {
        Ok(_) => Ok(MenuSelection::quit()),
        Err(error) => {
            let message = format!("failed to read a key press: {error}");
            Err(Error::Runtime(message))
        }
    };
}

/// Report whether the menu can read keys and draw at all.
fn is_interactive() -> bool {
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        return false;
    }
    let stdout = io::stdout();
    return stdout.is_terminal();
}

/// Owns raw mode and the alternate screen while the menu runs.
///
/// The guard is created before the alternate screen is entered, so a
/// failure at any later point still restores the terminal on drop.
struct TerminalGuard;

impl TerminalGuard {
    /// Enter the alternate screen, hide the cursor, and switch the
    /// terminal to raw mode.
    fn acquire() -> Result<Self, Error> {
        if let Err(error) = enable_raw_mode() {
            let message = format!("failed to enable raw mode: {error}");
            return Err(Error::Runtime(message));
        }
        let guard = TerminalGuard;
        let mut stdout = io::stdout();
        let entered = execute!(stdout, Hide, EnterAlternateScreen);
        return match entered {
            Ok(()) => Ok(guard),
            Err(error) => {
                let message =
                    format!("failed to enter the alternate screen: {error}");
                Err(Error::Runtime(message))
            }
        };
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // The terminal is restored best effort: a stream that fails here
        // cannot report its own failure either.
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
        return;
    }
}

/// Report whether `state` shows a form.
fn is_form(state: &AppState) -> bool {
    return match state {
        AppState::Form {
            ..
        } => true,
        AppState::Menu(_) => false,
    };
}

/// Return the highlighted row index for either screen.
///
/// A form keeps its command highlighted in the list, so returning from
/// it lands on the row the user opened.
fn selected_index(state: &AppState) -> usize {
    return match state {
        AppState::Menu(selected) => *selected,
        AppState::Form {
            command,
            ..
        } => index_of(*command),
    };
}

/// Return the number of input fields a command asks for.
///
/// `verify` only accepts a source, so its form has a single field; the
/// other commands need a source and a target.
fn field_count(command: MenuChoice) -> usize {
    return match command {
        MenuChoice::Verify => 1,
        MenuChoice::Pack | MenuChoice::Unpack => 2,
        MenuChoice::Quit => 0,
    };
}

/// Return the subcommand name used in the form line.
fn command_name(command: MenuChoice) -> &'static str {
    return match command {
        MenuChoice::Pack => "pack",
        MenuChoice::Verify => "verify",
        MenuChoice::Unpack => "unpack",
        MenuChoice::Quit => "quit",
    };
}

/// Return how many logo rows carry visible glyphs.
///
/// The wordmark constant ends with padding rows that hold only spaces.
/// Drawing them would leave blank lines between the artwork and the
/// version, so the frame stops at the last row with a glyph.
fn visible_logo_row_count() -> usize {
    let mut count: usize = LOGO_ROWS.len();
    while count > 0 {
        let index = count - 1;
        let row = match LOGO_ROWS.get(index) {
            Some(value) => *value,
            None => return count,
        };
        if row_has_glyph(row) {
            return count;
        }
        count = index;
    }
    return 0;
}

/// Report whether `row` holds anything other than whitespace.
fn row_has_glyph(row: &str) -> bool {
    for character in row.chars() {
        if !character.is_whitespace() {
            return true;
        }
    }
    return false;
}

/// Return how many characters `text` holds.
///
/// Iterator adapters are not used in this crate, so the width is counted
/// with an explicit loop instead of `chars().count()`.
fn char_width(text: &str) -> usize {
    let mut width: usize = 0;
    for _character in text.chars() {
        width += 1;
    }
    return width;
}

/// Render the screen for `state`.
///
/// The logo, version, slogan, command list, and hint are always drawn.
/// A form appends a separator, its editable command line, and a hint
/// below them, which keeps the menu visible while the form is open.
///
/// The logo is drawn as a horizontal rainbow gradient: each
/// non-whitespace character takes the next palette color and is
/// followed by a reset, so the color cannot bleed into the rest of the
/// frame. Whitespace keeps the layout uncolored and does not advance
/// the gradient.
///
/// Every line ends with CR LF. Raw mode turns off the terminal's output
/// post-processing, so a bare line feed moves down without returning to
/// the first column and each line would start where the previous ended.
pub fn build_frame(state: &AppState) -> String {
    let mut frame = build_menu_section(selected_index(state));
    if !is_form(state) {
        return frame;
    }
    frame.push_str(&separator_line());
    frame.push_str("\r\n");
    frame.push_str(&form_line(state));
    frame.push_str("\r\n");
    frame.push_str(form_hint(state));
    frame.push_str("\r\n");
    return frame;
}

/// Render the wordmark, version, slogan, commands, and hint.
///
/// The crate version sits directly below the wordmark, with no blank
/// line between them: the trailing all-space logo rows are not drawn.
/// It is right-aligned to the logo's width in italics, and the slogan
/// follows on the next line.
fn build_menu_section(selected: usize) -> String {
    let mut frame = String::new();
    let mut max_width: usize = 0;
    let row_count = visible_logo_row_count();
    let mut row_index: usize = 0;
    while row_index < row_count {
        let row = match LOGO_ROWS.get(row_index) {
            Some(value) => *value,
            None => break,
        };
        if row.len() > max_width {
            max_width = row.len();
        }
        let mut color_index: usize = 0;
        for character in row.chars() {
            if character.is_whitespace() {
                frame.push(character);
                continue;
            }
            let color = match RAINBOW.get(color_index % RAINBOW.len()) {
                Some(value) => *value,
                None => RESET,
            };
            frame.push_str(color);
            frame.push(character);
            frame.push_str(RESET);
            color_index += 1;
        }
        frame.push_str("\r\n");
        row_index += 1;
    }
    let version_text = format!("version: {}", env!("CARGO_PKG_VERSION"));
    let padding = max_width.saturating_sub(version_text.len());
    let mut written: usize = 0;
    while written < padding {
        frame.push(' ');
        written += 1;
    }
    frame.push_str(ITALIC);
    frame.push_str(&version_text);
    frame.push_str(RESET);
    frame.push_str("\r\n\r\n");
    frame.push_str(SLOGAN);
    frame.push_str("\r\n\r\n");
    let mut index: usize = 0;
    while index < MENU_ENTRIES.len() {
        frame.push_str(&render_entry(index, selected));
        frame.push_str("\r\n");
        index += 1;
    }
    frame.push_str("\r\n");
    frame.push_str(HINT);
    frame.push_str("\r\n");
    return frame;
}

/// Render the separator drawn above a form.
fn separator_line() -> String {
    let mut line = String::new();
    let mut width: usize = 0;
    while width < SEPARATOR_WIDTH {
        line.push(SEPARATOR_CHARACTER);
        width += 1;
    }
    return line;
}

/// Render the editable command line of the current form.
fn form_line(state: &AppState) -> String {
    let (command, active_field, source, target) = match state {
        AppState::Form {
            command,
            active_field,
            source_buffer,
            target_buffer,
        } => (command, active_field, source_buffer, target_buffer),
        AppState::Menu(_) => return String::new(),
    };
    let mut line = format!("van {}{FORM_SOURCE}", command_name(*command));
    line.push_str(&field_cell(*active_field, SOURCE_FIELD, source));
    if field_count(*command) >= 2 {
        line.push_str(FORM_TARGET);
        line.push_str(&field_cell(*active_field, TARGET_FIELD, target));
    }
    return line;
}

/// Render one buffer between brackets, highlighting it when active.
fn field_cell(active_field: usize, index: usize, buffer: &str) -> String {
    if active_field == index {
        return format!("[{ACTIVE_FIELD}{buffer}{RESET}]");
    }
    return format!("[{buffer}]");
}

/// Return the hint shown below the current form.
fn form_hint(state: &AppState) -> &'static str {
    let command = match state {
        AppState::Form {
            command,
            ..
        } => *command,
        AppState::Menu(_) => return "",
    };
    if field_count(command) < 2 {
        return SINGLE_FIELD_HINT;
    }
    return TWO_FIELD_HINT;
}

/// Return the column the cursor occupies inside the active field.
///
/// The column counts characters, not bytes, so a buffer with multi-byte
/// characters still puts the cursor after the text it holds.
fn form_cursor_column(state: &AppState) -> u16 {
    let (command, active_field, source, target) = match state {
        AppState::Form {
            command,
            active_field,
            source_buffer,
            target_buffer,
        } => (command, active_field, source_buffer, target_buffer),
        AppState::Menu(_) => return 0,
    };
    let mut column = "van ".len() + command_name(*command).len();
    column += FORM_SOURCE.len() + 1;
    column += char_width(source);
    if *active_field == TARGET_FIELD && field_count(*command) >= 2 {
        column += FIELD_GAP.len() + char_width(target);
    }
    let limit = usize::from(u16::MAX);
    if column > limit {
        return u16::MAX;
    }
    return column as u16;
}

/// Return the cursor position inside the form, when one is open.
///
/// The row counts the lines of the unchanged menu section plus the
/// separator, so the cursor follows the layout without hardcoding the
/// number of rows above it.
fn cursor_position(state: &AppState) -> Option<(u16, u16)> {
    if !is_form(state) {
        return None;
    }
    let menu = build_menu_section(selected_index(state));
    let mut rows: u16 = 0;
    for character in menu.chars() {
        if character == '\n' {
            rows = rows.saturating_add(1);
        }
    }
    let row = rows.saturating_add(1);
    let column = form_cursor_column(state);
    return Some((column, row));
}

/// Render one row, marking and highlighting it when it is selected.
fn render_entry(index: usize, selected: usize) -> String {
    let entry = match MENU_ENTRIES.get(index) {
        Some(value) => value,
        None => return String::new(),
    };
    let picked = index == selected;
    let indicator = match picked {
        true => "> ",
        false => "  ",
    };
    let cell = pad_label(entry.label);
    let mut line = match picked {
        // Only the selected row carries attributes.
        true => format!("{}", format!("{indicator}{cell}").bold().magenta()),
        false => format!("{indicator}{cell}"),
    };
    line.push_str(entry.description);
    return line;
}

/// Pad a command name to the column its description starts in.
fn pad_label(label: &str) -> String {
    let mut cell = String::from(label);
    let mut width: usize = label.len();
    while width < LABEL_WIDTH {
        cell.push(' ');
        width += 1;
    }
    return cell;
}

/// Draw the current screen and place the cursor for its state.
///
/// Clearing first keeps a shorter frame from leaving parts of the
/// previous screen behind. The cursor is hidden on the command list and
/// shown inside the active field while a form is open.
fn draw_screen(state: &AppState) -> Result<(), Error> {
    let frame = build_frame(state);
    let mut stdout = io::stdout();
    let queued =
        queue!(stdout, Clear(ClearType::All), MoveTo(0, 0), Print(frame));
    if let Err(error) = queued {
        let message = format!("failed to draw the menu: {error}");
        return Err(Error::Runtime(message));
    }
    let positioned = match cursor_position(state) {
        Some((column, row)) => queue!(stdout, Show, MoveTo(column, row)),
        None => queue!(stdout, Hide),
    };
    if let Err(error) = positioned {
        let message = format!("failed to place the cursor: {error}");
        return Err(Error::Runtime(message));
    }
    let flushed = stdout.flush();
    return match flushed {
        Ok(()) => Ok(()),
        Err(error) => {
            let message = format!("failed to flush the menu: {error}");
            Err(Error::Runtime(message))
        }
    };
}

/// Replace the screen with `text`, starting at the top-left corner.
fn draw_text(text: &str) -> Result<(), Error> {
    let mut stdout = io::stdout();
    let queued =
        queue!(stdout, Clear(ClearType::All), MoveTo(0, 0), Print(text));
    if let Err(error) = queued {
        let message = format!("failed to draw the menu: {error}");
        return Err(Error::Runtime(message));
    }
    let flushed = stdout.flush();
    return match flushed {
        Ok(()) => Ok(()),
        Err(error) => {
            let message = format!("failed to flush the menu: {error}");
            Err(Error::Runtime(message))
        }
    };
}

/// Return the command of the row at `index`.
///
/// The index always comes from the selection, so a miss can only happen
/// if the entries and the index disagree; quitting is the safe answer.
fn choice_at(index: usize) -> MenuChoice {
    let entry = MENU_ENTRIES.get(index);
    return match entry {
        Some(value) => value.choice,
        None => MenuChoice::Quit,
    };
}

/// Return the row index of `choice`.
fn index_of(choice: MenuChoice) -> usize {
    let mut index: usize = 0;
    while index < MENU_ENTRIES.len() {
        let entry = match MENU_ENTRIES.get(index) {
            Some(value) => value,
            None => return 0,
        };
        if entry.choice == choice {
            return index;
        }
        index += 1;
    }
    return 0;
}

/// Return the index above `current`, wrapping to the last row.
pub fn previous_index(current: usize) -> usize {
    if current == 0 {
        return MENU_ENTRIES.len() - 1;
    }
    return current - 1;
}

/// Return the index below `current`, wrapping to the first row.
pub fn next_index(current: usize) -> usize {
    let next = current + 1;
    if next >= MENU_ENTRIES.len() {
        return 0;
    }
    return next;
}
