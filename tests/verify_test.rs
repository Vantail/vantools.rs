//! Integration tests for the `verify` command.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command as ProcessCommand;
use std::process::Output;

use tempfile::TempDir;
use tempfile::tempdir;
use vantools::common::Error;
use vantools::common::command::Command;
use vantools::pack::PackCommand;
use vantools::verify::VerifyCommand;
use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// Name of the archives built by the helpers in this file.
const SAMPLE_ARCHIVE: &str = "sample.van";

#[test]
fn verify_accepts_a_packed_archive() {
    let root = temp_root();
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/test-plugin");
    let target = root.path().join("target");
    let pack = PackCommand::new(source, target.clone());
    let packed = pack.run();
    match packed {
        Ok(()) => {}
        Err(error) => panic!("pack failed: {error}"),
    }
    let archive = target.join("test-plugin-0.1.0.van");
    let verify = VerifyCommand::new(archive);
    let result = verify.run();
    match result {
        Ok(()) => {}
        Err(error) => panic!("verify failed: {error}"),
    }
}

#[test]
fn verify_accepts_a_minimal_archive() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_minimal_archive(&archive);
    let verify = VerifyCommand::new(archive);
    let result = verify.run();
    match result {
        Ok(()) => {}
        Err(error) => panic!("verify failed: {error}"),
    }
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
fn verify_accepts_regular_entries_with_permissions() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_permission_archive(&archive, "dist/app.ts", 0o644);
    let verify = VerifyCommand::new(archive);
    let result = verify.run();
    match result {
        Ok(()) => {}
        Err(error) => panic!("verify failed: {error}"),
    }
}

#[test]
fn verify_reports_a_missing_archive() {
    let root = temp_root();
    let archive = root.path().join("missing.van");
    let verify = VerifyCommand::new(archive.clone());
    let result = verify.run();
    let error = match result {
        Ok(()) => panic!("expected verification to be rejected"),
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
    assert!(stdout.contains("Archive: sample.van"));
    assert!(stdout.contains("├── dist"));
    assert!(stdout.contains("│   └── app.ts"));
    assert!(stdout.contains("└── manifest.json"));
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

/// Write an archive with a manifest and one `dist` file.
fn write_minimal_archive(path: &Path) {
    let entries: [(&str, &str); 2] =
        [("manifest.json", "{}"), ("dist/app.ts", "// placeholder\n")];
    write_archive(path, &entries);
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

/// Write an archive holding one entry with the given Unix permissions.
///
/// The permissions replace the lower mode bits only, so the entry stays
/// a regular file.
fn write_permission_archive(path: &Path, name: &str, mode: u32) {
    let mut writer = create_writer(path);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(mode);
    start_file(&mut writer, name, options);
    write_content(&mut writer, "// placeholder\n");
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

/// Run `verify` and return the error it must produce.
fn verify_error(archive: &Path) -> Error {
    let verify = VerifyCommand::new(archive.to_path_buf());
    let result = verify.run();
    return match result {
        Ok(()) => panic!("expected verification to be rejected"),
        Err(value) => value,
    };
}

/// Assert that `error` is a validation error with the expected message.
fn assert_validation_message(error: Error, expected: &str) {
    match error {
        Error::ValidationError(message) => assert_eq!(message, expected),
        other => panic!("unexpected error: {other}"),
    }
}
