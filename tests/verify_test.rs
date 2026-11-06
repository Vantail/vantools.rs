//! Integration tests for the `verify` command.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command as ProcessCommand;
use std::process::Output;

use tempfile::TempDir;
use tempfile::tempdir;
use vantools::common::Error;
use vantools::common::validation::escape_manifest;
use vantools::pack::PackCommand;
use vantools::verify::{VerifyCommand, VerifyReport};
use zip::CompressionMethod;
use zip::System;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// Name of the archives built by the helpers in this file.
const SAMPLE_ARCHIVE: &str = "sample.van";

/// Manifest used by the archives that are expected to be valid.
const SAMPLE_MANIFEST: &str = "{\"name\":\"sample-plugin\",\"version\":\"0.1.0\",\"entrypoint\":\"app.ts\"}";

/// Manifest whose text carries the JSON escape sequence for a control
/// character instead of the control character itself.
const ESCAPED_CONTROL_MANIFEST: &str = "{\"name\":\"sample-plugin\",\"version\":\"0.1.0\",\"entrypoint\":\"app.ts\",\"description\":\"\\u001b[31mred\"}";

/// Decompressed size of the entry written above the per-entry budget.
const OVERSIZED_ENTRY_BYTES: usize = 64 * 1024 * 1024 + 1;

/// Size of the chunk used to write the oversized entry.
const WRITE_CHUNK_BYTES: usize = 64 * 1024;

/// Number of `dist` entries written above the archive entry budget.
const OVERSIZED_ENTRY_COUNT: usize = 4096 + 1;

/// External attributes that mark an entry as a directory (`0o040755`).
const DIRECTORY_EXTERNAL_ATTRIBUTES: u32 = 0o040755 << 16;

#[test]
fn verify_accepts_a_packed_archive() {
    let root = temp_root();
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/test-plugin");
    let target = root.path().join("target");
    let pack = PackCommand::new(source, target.clone());
    let packed = pack.run();
    match packed {
        Ok(report) => assert_eq!(report.files_written, 2),
        Err(error) => panic!("pack failed: {error}"),
    }
    let archive = target.join("test-plugin-0.1.0.van");
    let report = verify_report(&archive);
    assert_eq!(report.source, archive);
    assert_eq!(report.manifest.entrypoint, "app.ts");
    assert_eq!(report.entry_names.len(), 2);
}

#[test]
fn verify_accepts_a_minimal_archive() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_minimal_archive(&archive);
    let report = verify_report(&archive);
    assert_eq!(report.source, archive);
    assert_eq!(report.manifest.entrypoint, "app.ts");
    assert_eq!(report.manifest_bytes.as_slice(), SAMPLE_MANIFEST.as_bytes());
    let mut names = report.entry_names;
    names.sort();
    let expected: Vec<String> =
        vec![String::from("dist/app.ts"), String::from("manifest.json")];
    assert_eq!(names, expected);
}

#[test]
fn verify_rejects_path_traversal() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(&archive, &[("../evil.txt", "escaped\n")]);
    let error = verify_error(&archive);
    assert_validation_message(
        error,
        "Path traversal segments are not allowed: ../evil.txt",
    );
}

#[test]
fn verify_rejects_absolute_paths() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(&archive, &[("/etc/passwd", "escaped\n")]);
    let error = verify_error(&archive);
    assert_validation_message(
        error,
        "Absolute paths or drive letters are not allowed: /etc/passwd",
    );
}

#[test]
fn verify_rejects_node_modules_at_any_depth() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(&archive, &[("dist/src/node_modules/index.js", "bundled\n")]);
    let error = verify_error(&archive);
    assert_validation_message(
        error,
        "Forbidden directory 'node_modules' found at: dist/src/node_modules/index.js",
    );
}

#[test]
fn verify_rejects_symlink_entries() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_symlink_archive(&archive, "dist/link.ts", "app.ts");
    let error = verify_error(&archive);
    assert_validation_message(
        error,
        "Symlinks are not allowed in the archive: dist/link.ts",
    );
}

#[test]
fn verify_rejects_directory_entries() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_directory_typed_archive(&archive);
    let error = verify_error(&archive);
    assert_validation_message(
        error,
        "Non-regular files are not allowed in the archive: dist/app.ts",
    );
}

#[test]
fn verify_rejects_duplicate_entry_names() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_duplicate_entry_archive(&archive);
    let error = verify_error(&archive);
    let expected = format!(
        "Duplicate archive entry names are not allowed: {}",
        archive.display()
    );
    assert_validation_message(error, &expected);
}

#[test]
fn verify_rejects_a_duplicate_manifest_entry() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_duplicate_manifest_archive(&archive);
    let error = verify_error(&archive);
    let expected = format!(
        "Duplicate archive entry names are not allowed: {}",
        archive.display()
    );
    assert_validation_message(error, &expected);
}

#[test]
fn verify_rejects_duplicate_names_differing_in_encoding() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_mixed_encoding_duplicate_archive(&archive);
    let error = verify_error(&archive);
    let expected = format!(
        "Duplicate archive entry names are not allowed: {}",
        archive.display()
    );
    assert_validation_message(error, &expected);
}

#[test]
fn verify_accepts_regular_entries_with_permissions() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    let entries: [(&str, &str); 2] =
        [("manifest.json", SAMPLE_MANIFEST), ("dist/app.ts", "// content\n")];
    write_archive_with_permissions(&archive, &entries, 0o644);
    let report = verify_report(&archive);
    assert_eq!(report.entry_names.len(), 2);
}

#[test]
fn verify_rejects_a_directory_source() {
    let root = temp_root();
    let archive = root.path().join("directory.van");
    let created = fs::create_dir(&archive);
    match created {
        Ok(()) => {}
        Err(error) => {
            panic!("failed to create {}: {error}", archive.display());
        }
    }
    let error = verify_error(&archive);
    let expected = format!(
        "Symlinks or non-regular files are not allowed: {}",
        archive.display()
    );
    assert_validation_message(error, &expected);
}

#[cfg(unix)]
#[test]
fn verify_rejects_a_device_source() {
    let archive = Path::new("/dev/null").to_path_buf();
    let error = verify_error(&archive);
    let expected = format!(
        "Symlinks or non-regular files are not allowed: {}",
        archive.display()
    );
    assert_validation_message(error, &expected);
}

#[test]
fn verify_rejects_corrupt_entry_data() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_stored_archive(&archive);
    replace_bytes(&archive, b"// content\n", b"// corrupt\n");
    let error = verify_error(&archive);
    let message = validation_message(error);
    assert!(
        message.contains("Failed to read entry data for dist/app.ts"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("Invalid checksum"),
        "unexpected error: {message}"
    );
}

#[test]
fn verify_rejects_entry_above_the_decompressed_budget() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_oversized_archive(&archive);
    let error = verify_error(&archive);
    let message = validation_message(error);
    assert_eq!(
        message,
        "Archive entry is larger than the 64 MiB limit: dist/app.ts"
    );
}

#[test]
fn verify_rejects_archive_above_the_entry_count_budget() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_many_entries_archive(&archive);
    let error = verify_error(&archive);
    let message = validation_message(error);
    let expected =
        format!("Archive has more than 4096 entries: {}", archive.display());
    assert_eq!(message, expected);
}

#[test]
fn verify_rejects_archive_without_manifest() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(&archive, &[("dist/app.ts", "// placeholder\n")]);
    let error = verify_error(&archive);
    let expected =
        format!("Missing 'manifest.json' in archive: {}", archive.display());
    assert_validation_message(error, &expected);
}

#[test]
fn verify_rejects_archive_without_dist() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    let entries: [(&str, &str); 2] =
        [("manifest.json", SAMPLE_MANIFEST), ("app.ts", "// placeholder\n")];
    write_archive(&archive, &entries);
    let error = verify_error(&archive);
    let expected =
        format!("Missing 'dist' directory in archive: {}", archive.display());
    assert_validation_message(error, &expected);
}

#[test]
fn verify_rejects_archive_without_entrypoint() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    let entries: [(&str, &str); 2] = [
        ("manifest.json", SAMPLE_MANIFEST),
        ("dist/other.ts", "// placeholder\n"),
    ];
    write_archive(&archive, &entries);
    let error = verify_error(&archive);
    assert_validation_message(
        error,
        "Entrypoint file not found in dist: dist/app.ts",
    );
}

#[test]
fn verify_rejects_invalid_manifest() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    let entries: [(&str, &str); 2] = [
        ("manifest.json", "{\"name\":\"sample-plugin\"}"),
        ("dist/app.ts", "// placeholder\n"),
    ];
    write_archive(&archive, &entries);
    let error = verify_error(&archive);
    let message = match error {
        Error::ValidationError(value) => value,
        other => panic!("unexpected error: {other}"),
    };
    assert!(
        message.contains("Manifest is not valid JSON"),
        "unexpected error: {message}"
    );
}

#[test]
fn verify_reports_a_missing_archive() {
    let root = temp_root();
    let archive = root.path().join("missing.van");
    let verify = VerifyCommand::new(archive.clone());
    let result = verify.run();
    let error = match result {
        Ok(_) => panic!("expected verification to be rejected"),
        Err(value) => value,
    };
    let message = match error {
        Error::Runtime(value) => value,
        other => panic!("unexpected error: {other}"),
    };
    let expected = format!("failed to open archive {}", archive.display());
    assert!(message.contains(&expected), "unexpected error: {message}");
}

#[test]
fn verify_cli_prints_a_tree() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_minimal_archive(&archive);
    let archive_path = match archive.to_str() {
        Some(value) => value,
        None => panic!("archive path is not valid Unicode"),
    };
    let output = run_van(&["verify", "--source", archive_path]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Verifying archive: sample.van"));
    assert!(stdout.contains("Check passed: Paths and ZIP structure"));
    assert!(
        stdout.contains("Check passed: manifest.json is present and valid")
    );
    assert!(
        stdout.contains(
            "Check passed: dist/ and entrypoint (app.ts) are present"
        )
    );
    // The manifest is pretty-printed, so the compact form must be gone.
    assert!(stdout.contains("\"name\": \"sample-plugin\""));
    assert!(!stdout.contains("\"name\":\"sample-plugin\""));
    assert!(stdout.contains("Archive Contents"));
    assert!(stdout.contains("\n    .\n"));
    assert!(stdout.contains("    ├── dist/"));
    assert!(stdout.contains("    │   └── app.ts"));
    assert!(stdout.contains("    └── manifest.json"));
    assert!(stdout.contains("Archive is valid."));
    // Piped output keeps the plain fallback: no colors, no log tags.
    assert!(!stdout.contains('\u{1b}'));
    assert!(!stdout.contains("[OK]"));
}

#[test]
fn verify_cli_prints_a_tree_for_shared_directories() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    let entries: [(&str, &str); 4] = [
        ("manifest.json", SAMPLE_MANIFEST),
        ("dist/app.ts", "// content\n"),
        ("dist/sub/deep.ts", "// deep\n"),
        ("dist-extra/other.ts", "// other\n"),
    ];
    write_archive(&archive, &entries);
    let archive_path = match archive.to_str() {
        Some(value) => value,
        None => panic!("archive path is not valid Unicode"),
    };
    let output = run_van(&["verify", "--source", archive_path]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = concat!(
        "    .\n",
        "    \u{251c}\u{2500}\u{2500} dist/\n",
        "    \u{2502}   \u{251c}\u{2500}\u{2500} app.ts\n",
        "    \u{2502}   \u{2514}\u{2500}\u{2500} sub/\n",
        "    \u{2502}       \u{2514}\u{2500}\u{2500} deep.ts\n",
        "    \u{251c}\u{2500}\u{2500} dist-extra/\n",
        "    \u{2502}   \u{2514}\u{2500}\u{2500} other.ts\n",
        "    \u{2514}\u{2500}\u{2500} manifest.json\n",
    );
    assert!(stdout.contains(expected), "unexpected tree: {stdout}");
}

#[test]
fn verify_cli_reuses_a_directory_matching_an_entry_name() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    let entries: [(&str, &str); 4] = [
        ("manifest.json", SAMPLE_MANIFEST),
        ("dist", "// placeholder\n"),
        ("dist/app.ts", "// content\n"),
        ("dist-extra/other.ts", "// other\n"),
    ];
    write_archive(&archive, &entries);
    let archive_path = match archive.to_str() {
        Some(value) => value,
        None => panic!("archive path is not valid Unicode"),
    };
    let output = run_van(&["verify", "--source", archive_path]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = concat!(
        "    .\n",
        "    \u{251c}\u{2500}\u{2500} dist/\n",
        "    \u{2502}   \u{2514}\u{2500}\u{2500} app.ts\n",
        "    \u{251c}\u{2500}\u{2500} dist-extra/\n",
        "    \u{2502}   \u{2514}\u{2500}\u{2500} other.ts\n",
        "    \u{2514}\u{2500}\u{2500} manifest.json\n",
    );
    assert!(stdout.contains(expected), "unexpected tree: {stdout}");
}

#[test]
fn manifest_text_escapes_control_characters() {
    let text = String::from("{\"name\":\"a\u{1b}[31m\u{7}\"}");
    let escaped = escape_manifest(&text);
    assert!(!escaped.contains('\u{1b}'));
    assert!(!escaped.contains('\u{7}'));
    assert!(escaped.contains("\\u{1b}"));
    assert!(!escaped.contains('\n'));
}

#[test]
fn manifest_text_keeps_line_breaks() {
    let text = String::from("{\n  \"name\": \"a\"\n}\n");
    let escaped = escape_manifest(&text);
    assert_eq!(escaped, text);
}

#[test]
fn verify_cli_keeps_manifest_escape_sequences_readable() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    let entries: [(&str, &str); 2] = [
        ("manifest.json", ESCAPED_CONTROL_MANIFEST),
        ("dist/app.ts", "// content\n"),
    ];
    write_archive(&archive, &entries);
    let archive_path = match archive.to_str() {
        Some(value) => value,
        None => panic!("archive path is not valid Unicode"),
    };
    let output = run_van(&["verify", "--source", archive_path]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\\u001b[31mred"));
    assert!(!stdout.contains('\u{1b}'));
}

#[test]
fn verify_cli_prints_help() {
    let output = run_van(&["verify", "-h"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: verify -s <source>"));
    assert!(stdout.contains("--source"));
}

#[test]
fn verify_cli_prints_version() {
    let output = run_van(&["verify", "-v"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected: String = format!("van {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(stdout, expected);
}

/// Create a temporary directory for one test.
fn temp_root() -> TempDir {
    let created = tempdir();
    return match created {
        Ok(value) => value,
        Err(error) => panic!("failed to create a temporary directory: {error}"),
    };
}

/// Run the compiled `van` binary with the given arguments.
fn run_van(arguments: &[&str]) -> Output {
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_van"));
    let launched = command.args(arguments).output();
    return match launched {
        Ok(output) => output,
        Err(error) => panic!("failed to run the van binary: {error}"),
    };
}

/// Write an archive with a valid manifest and one `dist` file.
fn write_minimal_archive(path: &Path) {
    let entries: [(&str, &str); 2] =
        [("manifest.json", SAMPLE_MANIFEST), ("dist/app.ts", "// content\n")];
    write_archive(path, &entries);
}

/// Write an archive whose entry content is stored unchanged.
///
/// Storing the content leaves the bytes of the entry in the file as
/// written, so a test can corrupt one entry without breaking any ZIP
/// offset or the central directory.
fn write_stored_archive(path: &Path) {
    let mut writer = create_writer(path);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored);
    start_file(&mut writer, "manifest.json", options);
    write_content(&mut writer, SAMPLE_MANIFEST);
    start_file(&mut writer, "dist/app.ts", options);
    write_content(&mut writer, "// content\n");
    finish_archive(writer, path);
}

/// Write an archive with one `dist` entry above the per-entry budget.
fn write_oversized_archive(path: &Path) {
    let mut writer = create_writer(path);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated);
    start_file(&mut writer, "manifest.json", options);
    write_content(&mut writer, SAMPLE_MANIFEST);
    start_file(&mut writer, "dist/app.ts", options);
    let chunk = [0u8; WRITE_CHUNK_BYTES];
    let mut remaining = OVERSIZED_ENTRY_BYTES;
    while remaining > 0 {
        let count = remaining.min(WRITE_CHUNK_BYTES);
        let slice = match chunk.get(..count) {
            Some(value) => value,
            None => panic!("chunk exceeds the buffer"),
        };
        let written = writer.write_all(slice);
        match written {
            Ok(()) => {}
            Err(error) => panic!("failed to write entry content: {error}"),
        }
        remaining -= count;
    }
    finish_archive(writer, path);
}

/// Write an archive whose entry count is above the entry budget.
fn write_many_entries_archive(path: &Path) {
    let mut writer = create_writer(path);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated);
    start_file(&mut writer, "manifest.json", options);
    write_content(&mut writer, SAMPLE_MANIFEST);
    let mut index = 0;
    while index < OVERSIZED_ENTRY_COUNT {
        let name = format!("dist/file{index}.ts");
        start_file(&mut writer, &name, options);
        write_content(&mut writer, "// content\n");
        index += 1;
    }
    finish_archive(writer, path);
}

/// Replace the first occurrence of `old` with `new` inside `path`.
///
/// Both slices must have the same length so every ZIP offset stays
/// valid and only the matched bytes change.
fn replace_bytes(path: &Path, old: &[u8], new: &[u8]) {
    if old.len() != new.len() {
        panic!("replacement must preserve the length");
    }
    let loaded = fs::read(path);
    let mut bytes = match loaded {
        Ok(value) => value,
        Err(error) => panic!("failed to read {}: {error}", path.display()),
    };
    let mut start = 0;
    loop {
        let end = start + old.len();
        let candidate = match bytes.get(start..end) {
            Some(value) => value,
            None => {
                panic!("content to replace not found in {}", path.display())
            }
        };
        if candidate == old {
            let target = match bytes.get_mut(start..end) {
                Some(value) => value,
                None => {
                    panic!("failed to replace bytes in {}", path.display());
                }
            };
            target.copy_from_slice(new);
            break;
        }
        start += 1;
    }
    let written = fs::write(path, &bytes);
    match written {
        Ok(()) => {}
        Err(error) => panic!("failed to write {}: {error}", path.display()),
    }
}

/// Replace every occurrence of `old` with `new` inside `path`.
///
/// Both slices must have the same length so every ZIP offset stays
/// valid and only the matched bytes change.
fn replace_all_bytes(path: &Path, old: &[u8], new: &[u8]) {
    if old.len() != new.len() {
        panic!("replacement must preserve the length");
    }
    let loaded = fs::read(path);
    let mut bytes = match loaded {
        Ok(value) => value,
        Err(error) => panic!("failed to read {}: {error}", path.display()),
    };
    let mut start = 0;
    let mut replaced = false;
    loop {
        let end = start + old.len();
        let candidate = match bytes.get(start..end) {
            Some(value) => value,
            None => break,
        };
        if candidate == old {
            let target = match bytes.get_mut(start..end) {
                Some(value) => value,
                None => {
                    panic!("failed to replace bytes in {}", path.display());
                }
            };
            target.copy_from_slice(new);
            replaced = true;
        }
        start += 1;
    }
    if !replaced {
        panic!("content to replace not found in {}", path.display());
    }
    let written = fs::write(path, &bytes);
    match written {
        Ok(()) => {}
        Err(error) => panic!("failed to write {}: {error}", path.display()),
    }
}

/// Write an archive holding `entries` in the given order.
fn write_archive(path: &Path, entries: &[(&str, &str)]) {
    let mut writer = create_writer(path);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated);
    for (name, content) in entries {
        start_file(&mut writer, name, options);
        write_content(&mut writer, content);
    }
    finish_archive(writer, path);
}

/// Write an archive whose `dist/app.ts` name appears twice.
///
/// The ZIP writer refuses to add one name twice, so the second entry is
/// written under a decoy name of the same length and its name bytes are
/// rewritten afterwards. Every ZIP offset stays valid because the name
/// keeps its length, and no checksum covers an entry name.
fn write_duplicate_entry_archive(path: &Path) {
    let entries: [(&str, &str); 3] = [
        ("manifest.json", SAMPLE_MANIFEST),
        ("dist/app.ts", "// content\n"),
        ("dist/appXts", "// shadowing content\n"),
    ];
    write_archive(path, &entries);
    replace_all_bytes(path, b"appXts", b"app.ts");
}

/// Write an archive whose `manifest.json` name appears twice.
///
/// The second entry carries a different but still valid manifest, so a
/// consumer that resolves the name differently would validate a
/// different plugin than the first entry describes.
fn write_duplicate_manifest_archive(path: &Path) {
    let second_manifest = concat!(
        "{\"name\":\"other-plugin\",",
        "\"version\":\"9.9.9\",",
        "\"entrypoint\":\"app.ts\"}"
    );
    let entries: [(&str, &str); 3] = [
        ("manifest.json", SAMPLE_MANIFEST),
        ("dist/app.ts", "// content\n"),
        ("manifestXjson", second_manifest),
    ];
    write_archive(path, &entries);
    replace_all_bytes(path, b"manifestXjson", b"manifest.json");
}

/// Write an archive that reports one decoded name twice.
///
/// The first `dist/café.ts` is stored as UTF-8 and the second one is
/// stored under a decoy name whose bytes are rewritten to CP437, so the
/// two central directory names differ while the reader decodes both to
/// `dist/café.ts`. A byte-level duplicate check cannot see this case
/// because the raw names differ.
fn write_mixed_encoding_duplicate_archive(path: &Path) {
    let entries: [(&str, &str); 4] = [
        ("manifest.json", SAMPLE_MANIFEST),
        ("dist/app.ts", "// content\n"),
        ("dist/café.ts", "// utf8 content\n"),
        ("dist/cafX.ts", "// cp437 content\n"),
    ];
    write_archive(path, &entries);
    replace_all_bytes(path, b"cafX", b"caf\x82");
}

/// Write an archive holding `entries` with the given Unix permissions.
///
/// The permissions replace the lower mode bits only, so every entry
/// stays a regular file.
fn write_archive_with_permissions(
    path: &Path,
    entries: &[(&str, &str)],
    mode: u32,
) {
    let mut writer = create_writer(path);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(mode);
    for (name, content) in entries {
        start_file(&mut writer, name, options);
        write_content(&mut writer, content);
    }
    finish_archive(writer, path);
}

/// Write an archive whose single entry is marked as a symbolic link.
fn write_symlink_archive(path: &Path, name: &str, target: &str) {
    let mut writer = create_writer(path);
    let options = SimpleFileOptions::default();
    let added = writer.add_symlink(name, target, options);
    match added {
        Ok(()) => {}
        Err(error) => panic!("failed to add the symlink {name}: {error}"),
    }
    finish_archive(writer, path);
}

/// Write an archive whose `dist/app.ts` entry declares a directory.
///
/// Every entry declares a Unix origin so that its mode is read back the
/// same way on every platform, instead of depending on the system the
/// archive was written on.
fn write_directory_typed_archive(path: &Path) {
    let mut writer = create_writer(path);
    let regular = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .system(System::Unix);
    start_file(&mut writer, "manifest.json", regular);
    write_content(&mut writer, SAMPLE_MANIFEST);
    let directory = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .system(System::Unix)
        .external_attributes(DIRECTORY_EXTERNAL_ATTRIBUTES);
    start_file(&mut writer, "dist/app.ts", directory);
    write_content(&mut writer, "// content\n");
    finish_archive(writer, path);
}

/// Create a ZIP writer for a new archive file.
fn create_writer(path: &Path) -> ZipWriter<fs::File> {
    let created = fs::File::create(path);
    let file = match created {
        Ok(value) => value,
        Err(error) => {
            panic!("failed to create {}: {error}", path.display());
        }
    };
    return ZipWriter::new(file);
}

/// Start one entry, failing the test when it cannot be added.
fn start_file(
    writer: &mut ZipWriter<fs::File>,
    name: &str,
    options: SimpleFileOptions,
) {
    let started = writer.start_file(name, options);
    match started {
        Ok(()) => {}
        Err(error) => panic!("failed to add {name}: {error}"),
    }
}

/// Write the content of one entry, failing the test on error.
fn write_content(writer: &mut ZipWriter<fs::File>, content: &str) {
    let written = writer.write_all(content.as_bytes());
    match written {
        Ok(()) => {}
        Err(error) => panic!("failed to write entry content: {error}"),
    }
}

/// Finish the archive, failing the test on error.
fn finish_archive(writer: ZipWriter<fs::File>, path: &Path) {
    let finished = writer.finish();
    match finished {
        Ok(_) => {}
        Err(error) => {
            panic!("failed to finish {}: {error}", path.display());
        }
    }
}

/// Run `verify` and return the report it must produce.
fn verify_report(archive: &Path) -> VerifyReport {
    let verify = VerifyCommand::new(archive.to_path_buf());
    let result = verify.run();
    return match result {
        Ok(value) => value,
        Err(error) => panic!("verify failed: {error}"),
    };
}

/// Run `verify` and return the error it must produce.
fn verify_error(archive: &Path) -> Error {
    let verify = VerifyCommand::new(archive.to_path_buf());
    let result = verify.run();
    return match result {
        Ok(_) => panic!("expected verification to be rejected"),
        Err(value) => value,
    };
}

/// Return the message of a validation error.
fn validation_message(error: Error) -> String {
    return match error {
        Error::ValidationError(value) => value,
        other => panic!("unexpected error: {other}"),
    };
}

/// Assert that `error` is a validation error with the expected message.
fn assert_validation_message(error: Error, expected: &str) {
    match error {
        Error::ValidationError(message) => assert_eq!(message, expected),
        other => panic!("unexpected error: {other}"),
    }
}
