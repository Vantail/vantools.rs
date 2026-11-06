//! Report formatting for a successful archive verification.
//!
//! The report is written to stdout only after every check passed, so an
//! invalid archive never produces a partial report. Untrusted values,
//! such as the manifest bytes and the entry names, are escaped before
//! they reach the terminal.

use std::io;
use std::io::Write;
use std::path::Path;

use crate::common::Error;
use crate::common::output::{write_line, write_text};
use crate::common::validation::{escape_manifest, escape_path, escape_text};
use crate::tui::tree::render_tree_lines;

use super::ArchiveCheck;

/// Print the result of a successful verification.
///
/// The collected entry names are sorted in place by the tree renderer,
/// so the report never holds a second copy of them.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when stdout cannot be written.
pub(super) fn print_report(
    source: &Path,
    check: &mut ArchiveCheck,
) -> Result<(), Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let header = format!("Verifying archive: {}", archive_label(source));
    write_line(&mut handle, &header, "output")?;
    write_line(
        &mut handle,
        "  [OK] Paths and ZIP structure are valid.",
        "output",
    )?;
    write_line(
        &mut handle,
        "  [OK] manifest.json is present and valid.",
        "output",
    )?;
    let entrypoint = escape_text(&check.manifest.entrypoint);
    let entrypoint_line =
        format!("  [OK] dist/ and entrypoint ({entrypoint}) are present.");
    write_line(&mut handle, &entrypoint_line, "output")?;
    write_line(&mut handle, "", "output")?;
    write_line(&mut handle, "--- manifest.json ---", "output")?;
    write_manifest(&mut handle, &check.manifest_bytes)?;
    write_line(&mut handle, "---------------------", "output")?;
    write_line(&mut handle, "", "output")?;
    write_line(&mut handle, "Archive Contents:", "output")?;
    for line in render_tree_lines(&mut check.names) {
        write_line(&mut handle, &line, "output")?;
    }
    write_line(&mut handle, "", "output")?;
    return write_line(&mut handle, "Archive is valid.", "output");
}

/// Print the manifest bytes, ending with one newline.
///
/// The archived manifest is untrusted, so its control characters are
/// rendered as visible escapes before they reach the terminal: printing
/// the bytes verbatim would let a crafted manifest emit terminal escape
/// sequences, overwrite the line, or forge log lines in captured output.
/// Line breaks are kept so a formatted manifest stays readable, and a
/// printable manifest is rendered unchanged.
fn write_manifest<W>(handle: &mut W, bytes: &[u8]) -> Result<(), Error>
where
    W: Write,
{
    let text = String::from_utf8_lossy(bytes);
    let escaped = escape_manifest(&text);
    write_text(handle, &escaped, "output")?;
    if escaped.ends_with('\n') {
        return Ok(());
    }
    return write_text(handle, "\n", "output");
}

/// Render the archive file name used in the report header.
fn archive_label(source: &Path) -> String {
    let name = match source.file_name() {
        Some(value) => value,
        None => return escape_path(source),
    };
    return escape_path(Path::new(name));
}
