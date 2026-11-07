//! Integration tests for the `unpack` command.

use std::fs;
use std::io::Write;
use std::path::Path;

use tempfile::TempDir;
use tempfile::tempdir;
use vantools::common::Error;
use vantools::common::validation::escape_path;
use vantools::unpack::{UnpackCommand, UnpackReport};
use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// Name of the archives built by the helpers in this file.
const SAMPLE_ARCHIVE: &str = "sample.van";

/// Manifest used by the archives that are expected to be valid.
const SAMPLE_MANIFEST: &str = "{\"name\":\"sample-plugin\",\"version\":\"0.1.0\",\"entrypoint\":\"app.ts\"}";

#[test]
fn unpack_rejects_an_archive_without_a_manifest() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(&archive, &[("dist/app.ts", "// content\n")]);
    let target = root.path().join("out");
    let error = unpack_error(&archive, &target);
    let expected = format!(
        "Missing 'manifest.json' in archive: {}",
        escape_path(&archive)
    );
    assert_validation_message(error, &expected);
    assert!(
        !target.exists(),
        "a validation failure created the target directory"
    );
}

#[test]
fn unpack_rejects_path_traversal_without_touching_the_target() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(
        &archive,
        &[
            ("manifest.json", SAMPLE_MANIFEST),
            ("dist/../evil.txt", "escaped\n"),
        ],
    );
    let target = root.path().join("out");
    let error = unpack_error(&archive, &target);
    assert_validation_message(
        error,
        "Path traversal segments are not allowed: dist/../evil.txt",
    );
    assert!(
        !target.exists(),
        "a validation failure created the target directory"
    );
}

#[test]
fn unpack_reports_the_validated_manifest() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(
        &archive,
        &[("manifest.json", SAMPLE_MANIFEST), ("dist/app.ts", "// content\n")],
    );
    let target = root.path().join("out");
    let report = unpack_report(&archive, &target);
    assert_eq!(report.archive, archive);
    assert_eq!(report.target, target);
    assert_eq!(report.manifest.entrypoint, "app.ts");
    // Validation alone extracts nothing yet.
    assert!(!target.exists(), "validation created the target directory");
}

/// Create a temporary directory for one test.
fn temp_root() -> TempDir {
    let created = tempdir();
    return match created {
        Ok(value) => value,
        Err(error) => panic!("failed to create a temporary directory: {error}"),
    };
}

/// Write an archive holding `entries` in the given order.
fn write_archive(path: &Path, entries: &[(&str, &str)]) {
    let created = fs::File::create(path);
    let file = match created {
        Ok(value) => value,
        Err(error) => panic!("failed to create {}: {error}", path.display()),
    };
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated);
    for (name, content) in entries {
        let started = writer.start_file(*name, options);
        match started {
            Ok(()) => {}
            Err(error) => panic!("failed to add {name}: {error}"),
        }
        let written = writer.write_all(content.as_bytes());
        match written {
            Ok(()) => {}
            Err(error) => panic!("failed to write {name}: {error}"),
        }
    }
    let finished = writer.finish();
    match finished {
        Ok(_) => {}
        Err(error) => panic!("failed to finish {}: {error}", path.display()),
    }
    return;
}

/// Run `unpack` and return the report it must produce.
fn unpack_report(archive: &Path, target: &Path) -> UnpackReport {
    let command =
        UnpackCommand::new(archive.to_path_buf(), target.to_path_buf());
    let result = command.run();
    return match result {
        Ok(value) => value,
        Err(error) => panic!("unpack failed: {error}"),
    };
}

/// Run `unpack` and return the error it must produce.
fn unpack_error(archive: &Path, target: &Path) -> Error {
    let command =
        UnpackCommand::new(archive.to_path_buf(), target.to_path_buf());
    let result = command.run();
    return match result {
        Ok(_) => panic!("expected the archive to be rejected"),
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
