//! Integration tests for the `unpack` command.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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
    assert_no_temp_entries(root.path());
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
    assert_no_temp_entries(root.path());
}

#[test]
fn unpack_extracts_the_source_layout() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(
        &archive,
        &[
            ("manifest.json", SAMPLE_MANIFEST),
            ("dist/app.ts", "// entrypoint content\n"),
        ],
    );
    let target = root.path().join("out");
    let report = unpack_report(&archive, &target);
    assert_eq!(report.archive, archive);
    assert_eq!(report.target, target);
    assert_eq!(report.manifest.entrypoint, "app.ts");
    assert_eq!(report.files_written, 2);
    assert!(target.is_dir());
    assert_file_content(&target.join("manifest.json"), SAMPLE_MANIFEST);
    assert_file_content(
        &target.join("dist").join("app.ts"),
        "// entrypoint content\n",
    );
    assert_no_temp_entries(root.path());
}

#[test]
fn unpack_extracts_nested_dist_directories() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(
        &archive,
        &[
            ("manifest.json", SAMPLE_MANIFEST),
            ("dist/app.ts", "// entrypoint\n"),
            ("dist/src/util.ts", "// helper\n"),
        ],
    );
    let target = root.path().join("out");
    let report = unpack_report(&archive, &target);
    assert_eq!(report.files_written, 3);
    assert_file_content(
        &target.join("dist").join("src").join("util.ts"),
        "// helper\n",
    );
    assert_no_temp_entries(root.path());
}

#[test]
fn unpack_rejects_an_existing_target_directory() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(
        &archive,
        &[("manifest.json", SAMPLE_MANIFEST), ("dist/app.ts", "// content\n")],
    );
    let target = root.path().join("out");
    create_dir(&target);
    let sentinel = target.join("keep.txt");
    write_file(&sentinel, "user data\n");
    let error = unpack_error(&archive, &target);
    let expected =
        format!("Target directory already exists: {}", escape_path(&target));
    assert_validation_message(error, &expected);
    assert_file_content(&sentinel, "user data\n");
    assert_no_temp_entries(root.path());
}

#[test]
fn unpack_clears_a_leftover_temporary_directory() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(
        &archive,
        &[("manifest.json", SAMPLE_MANIFEST), ("dist/app.ts", "// content\n")],
    );
    let target = root.path().join("out");
    let leftover = temporary_path(&target);
    create_dir(&leftover);
    let stale = leftover.join("stale.txt");
    write_file(&stale, "stale leftover\n");
    let report = unpack_report(&archive, &target);
    assert_eq!(report.files_written, 2);
    assert!(!leftover.exists(), "the temporary directory was not removed");
    assert!(target.join("dist").join("app.ts").is_file());
    assert_no_temp_entries(root.path());
}

/// Create a temporary directory for one test.
fn temp_root() -> TempDir {
    let created = tempdir();
    return match created {
        Ok(value) => value,
        Err(error) => panic!("failed to create a temporary directory: {error}"),
    };
}

/// Derive the temporary directory a run uses for `target`.
///
/// The writer keeps the process id in the name, so a test can place a
/// leftover entry at exactly that path.
fn temporary_path(target: &Path) -> PathBuf {
    let name = match target.file_name() {
        Some(value) => value.to_owned(),
        None => panic!("target has no file name: {}", target.display()),
    };
    let mut temp_name = name;
    temp_name.push(format!(".tmp-{}", std::process::id()));
    return target.with_file_name(temp_name);
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

/// Create a directory, failing the test when it cannot be created.
fn create_dir(path: &Path) {
    let created = fs::create_dir_all(path);
    match created {
        Ok(()) => {}
        Err(error) => panic!("failed to create {}: {error}", path.display()),
    }
}

/// Write a file, failing the test when it cannot be written.
fn write_file(path: &Path, contents: &str) {
    let written = fs::write(path, contents);
    match written {
        Ok(()) => {}
        Err(error) => panic!("failed to write {}: {error}", path.display()),
    }
}

/// Read a file and compare it with the expected text.
fn assert_file_content(path: &Path, expected: &str) {
    let content = fs::read(path);
    let bytes = match content {
        Ok(value) => value,
        Err(error) => panic!("failed to read {}: {error}", path.display()),
    };
    assert_eq!(
        bytes.as_slice(),
        expected.as_bytes(),
        "unexpected content in {}",
        path.display()
    );
}

/// Assert that `parent` holds no leftover temporary directory.
fn assert_no_temp_entries(parent: &Path) {
    let listed = fs::read_dir(parent);
    let entries = match listed {
        Ok(value) => value,
        Err(error) => {
            panic!("failed to read {}: {error}", parent.display());
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(value) => value,
            Err(error) => {
                panic!(
                    "failed to read an entry of {}: {error}",
                    parent.display()
                );
            }
        };
        let name = entry.file_name();
        let rendered = String::from(name.to_string_lossy());
        assert!(
            !rendered.contains(".tmp-"),
            "leftover temporary directory: {rendered}"
        );
    }
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
