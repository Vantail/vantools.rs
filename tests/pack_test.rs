//! Integration tests for the `pack` command and the path validators.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use tempfile::TempDir;
use tempfile::tempdir;
use vantools::common::Error;
use vantools::common::command::Command;
use vantools::common::validation::{
    validate_archive_entry, validate_no_node_modules,
};
use vantools::pack::PackCommand;

/// Name of the archive the example plugin must produce.
const EXPECTED_ARCHIVE: &str = "test-plugin-0.1.0.van";

/// Manifest used by the temporary plugin sources in these tests.
const TEST_MANIFEST: &str =
    "{\"name\":\"bad-plugin\",\"version\":\"0.1.0\",\"entrypoint\":\"app.ts\"}";

/// Name of the archive built from [`TEST_MANIFEST`].
const BAD_ARCHIVE: &str = "bad-plugin-0.1.0.van";

#[test]
fn pack_creates_van_archive() {
    let created = tempdir();
    let target_dir = match created {
        Ok(value) => value,
        Err(error) => panic!("failed to create a temporary directory: {error}"),
    };
    let source = PathBuf::from("examples/test-plugin");
    let command = PackCommand::new(source, target_dir.path().to_path_buf());
    let result = command.run();
    match result {
        Ok(()) => {}
        Err(error) => panic!("pack failed: {error}"),
    }
    let archive = target_dir.path().join(EXPECTED_ARCHIVE);
    assert!(archive.is_file());
}

#[test]
fn pack_rejects_node_modules_at_any_depth() {
    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let forbidden = source.join("dist").join("src").join("node_modules");
    create_dir(&forbidden);
    write_file(&forbidden.join("index.js"), "// bundled dependency\n");
    let target = root.path().join("target");
    let error = pack_error(&source, &target);
    assert_validation_message(
        error,
        "Forbidden directory 'node_modules' found at: dist/src/node_modules",
    );
    assert!(!target.join(BAD_ARCHIVE).exists());
}

#[cfg(unix)]
#[test]
fn pack_rejects_symlinks() {
    use std::os::unix::fs::symlink;

    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let link_path = source.join("dist").join("link.ts");
    let linked = symlink("app.ts", &link_path);
    match linked {
        Ok(()) => {}
        Err(error) => panic!("failed to create a symlink: {error}"),
    }
    let target = root.path().join("target");
    let error = pack_error(&source, &target);
    let expected = format!(
        "Symlinks or non-regular files are not allowed: {}",
        link_path.display()
    );
    assert_validation_message(error, &expected);
    assert!(!target.join(BAD_ARCHIVE).exists());
}

#[test]
fn validate_archive_entry_rejects_backslashes() {
    let error = entry_error("dist\\app.ts");
    assert_validation_message(
        error,
        "Backslashes are not allowed in paths: dist\\app.ts",
    );
}

#[test]
fn validate_archive_entry_rejects_absolute_paths() {
    let error = entry_error("/etc/passwd");
    assert_validation_message(
        error,
        "Absolute paths or drive letters are not allowed: /etc/passwd",
    );
}

#[test]
fn validate_archive_entry_rejects_drive_letters() {
    let error = entry_error("C:app.ts");
    assert_validation_message(
        error,
        "Absolute paths or drive letters are not allowed: C:app.ts",
    );
}

#[test]
fn validate_archive_entry_rejects_empty_segments() {
    let error = entry_error("dist//app.ts");
    assert_validation_message(
        error,
        "Absolute paths or drive letters are not allowed: dist//app.ts",
    );
}

#[test]
fn validate_archive_entry_rejects_parent_segments() {
    let error = entry_error("dist/../app.ts");
    assert_validation_message(
        error,
        "Path traversal segments are not allowed: dist/../app.ts",
    );
}

#[test]
fn validate_archive_entry_rejects_current_segments() {
    let error = entry_error("dist/./app.ts");
    assert_validation_message(
        error,
        "Path traversal segments are not allowed: dist/./app.ts",
    );
}

#[test]
fn validate_archive_entry_rejects_non_nfc_paths() {
    let error = entry_error("dist/cafe\u{301}.ts");
    assert_validation_message(
        error,
        "Path is not in Unicode NFC format: dist/cafe\u{301}.ts",
    );
}

#[test]
fn validate_archive_entry_accepts_valid_paths() {
    let ascii = validate_archive_entry("dist/src/app.ts");
    match ascii {
        Ok(()) => {}
        Err(error) => panic!("valid path was rejected: {error}"),
    }
    let precomposed = validate_archive_entry("dist/caf\u{e9}.ts");
    match precomposed {
        Ok(()) => {}
        Err(error) => panic!("valid NFC path was rejected: {error}"),
    }
}

#[test]
fn validate_no_node_modules_rejects_forbidden_component() {
    let result = validate_no_node_modules(Path::new("dist/node_modules"));
    let error = match result {
        Ok(()) => panic!("expected node_modules to be rejected"),
        Err(value) => value,
    };
    assert_validation_message(
        error,
        "Forbidden directory 'node_modules' found at: dist/node_modules",
    );
}

#[test]
fn validate_no_node_modules_accepts_other_paths() {
    let result = validate_no_node_modules(Path::new("dist/src/app.ts"));
    match result {
        Ok(()) => {}
        Err(error) => panic!("valid path was rejected: {error}"),
    }
}

/// Create a temporary directory for one test.
fn temp_root() -> TempDir {
    let created = tempdir();
    return match created {
        Ok(value) => value,
        Err(error) => panic!("failed to create a temporary directory: {error}"),
    };
}

/// Create a plugin source with a manifest and `dist/app.ts`.
fn create_source(root: &Path) {
    let dist = root.join("dist");
    create_dir(&dist);
    write_file(&root.join("manifest.json"), TEST_MANIFEST);
    write_file(&dist.join("app.ts"), "// placeholder\n");
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

/// Run `pack` and return the error it must produce.
fn pack_error(source: &Path, target: &Path) -> Error {
    let command = PackCommand::new(source.to_path_buf(), target.to_path_buf());
    let result = command.run();
    return match result {
        Ok(()) => panic!("expected packing to be rejected"),
        Err(value) => value,
    };
}

/// Validate an entry name that must be rejected.
fn entry_error(path_str: &str) -> Error {
    let result = validate_archive_entry(path_str);
    return match result {
        Ok(()) => panic!("expected the entry to be rejected"),
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
