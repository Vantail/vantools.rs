//! Report formatting for a successful archive verification.
//!
//! The report is written to stdout only after every check passed, so an
//! invalid archive never produces a partial report. Every validation
//! step goes through the log helpers, the manifest is re-serialized as
//! indented JSON so it reads like a code block, and the tree renderer
//! lists the entry names as a plain indented tree. Untrusted values,
//! such as the manifest bytes and the entry names, are escaped before
//! they reach the terminal.

use std::io;
use std::path::Path;

use serde_json::Value;
use vantools::common::Error;
use vantools::common::tree::render_tree_lines;
use vantools::common::validation::{escape_manifest, escape_path, escape_text};
use vantools::verify::VerifyReport;

use super::output::write_line;
use super::output::{log_block, log_info, log_ok, log_start};

/// Print the result of a successful verification.
///
/// Each check is one log line, the manifest follows as an indented JSON
/// block, and the archive contents trail as an indented tree. The
/// collected entry names are sorted in place by the tree renderer, so
/// the report never holds a second copy of them.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when stdout cannot be written.
pub fn print_report(report: &mut VerifyReport) -> Result<(), Error> {
    let label = archive_label(&report.source);
    log_start(&format!("Verifying archive: {label}"));
    log_ok("Check passed", "Paths and ZIP structure");
    log_ok("Check passed", "manifest.json is present and valid");
    let entrypoint = escape_text(&report.manifest.entrypoint);
    let entrypoint_check =
        format!("dist/ and entrypoint ({entrypoint}) are present");
    log_ok("Check passed", &entrypoint_check);
    log_info("manifest.json", "");
    print_manifest(&report.manifest_bytes)?;
    log_info("Archive Contents", "");
    {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        for line in render_tree_lines(&mut report.entry_names) {
            write_line(&mut handle, &line, "output")?;
        }
        write_line(&mut handle, "", "output")?;
    }
    log_ok("Archive is valid.", "");
    return Ok(());
}

/// Print the archived manifest as a block, followed by one blank line.
///
/// The archived bytes are untrusted, so they are parsed and written back
/// out by `serde_json` instead of being dumped verbatim: control
/// characters end up escaped, so a crafted manifest cannot emit terminal
/// escape sequences or forge log lines. A manifest that does not parse
/// as JSON falls back to the escaped text, which keeps the report
/// complete.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when stdout cannot be written.
fn print_manifest(bytes: &[u8]) -> Result<(), Error> {
    let text = render_manifest(bytes);
    log_block(&text);
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    return write_line(&mut handle, "", "output");
}

/// Render the manifest bytes as indented JSON.
fn render_manifest(bytes: &[u8]) -> String {
    let parsed = serde_json::from_slice::<Value>(bytes);
    let value = match parsed {
        Ok(value) => value,
        Err(_) => return escaped_manifest(bytes),
    };
    let pretty = serde_json::to_string_pretty(&value);
    return match pretty {
        Ok(text) => text,
        Err(_) => escaped_manifest(bytes),
    };
}

/// Render the manifest bytes as escaped text, keeping their line breaks.
fn escaped_manifest(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    return escape_manifest(&text);
}

/// Render the archive file name used in the report header.
fn archive_label(source: &Path) -> String {
    let name = match source.file_name() {
        Some(value) => value,
        None => return escape_path(source),
    };
    return escape_path(Path::new(name));
}
