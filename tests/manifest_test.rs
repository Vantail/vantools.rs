//! Integration tests for `manifest.json` validation.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use tempfile::TempDir;
use tempfile::tempdir;
use vantools::common::Error;
use vantools::common::command::Command;
use vantools::common::manifest::Manifest;
use vantools::pack::PackCommand;

/// A manifest that passes every validation rule.
const VALID_MANIFEST: &str = "{\"name\":\"example-plugin\",\
\"version\":\"1.2.3\",\"entrypoint\":\"src/app.ts\"}";

#[test]
fn valid_manifest_parses() {
    let root = temp_root();
    write_file(&root.path().join("manifest.json"), VALID_MANIFEST.as_bytes());
    let manifest = expect_manifest(root.path());
    assert_eq!(manifest.name, "example-plugin");
    assert_eq!(manifest.version, "1.2.3");
    assert_eq!(manifest.entrypoint, "src/app.ts");
}

#[test]
fn missing_name_field_is_reported_as_json_error() {
    let root = temp_root();
    let content = "{\"version\":\"1.2.3\",\"entrypoint\":\"app.ts\"}";
    write_file(&root.path().join("manifest.json"), content.as_bytes());
    let message = manifest_message(root.path());
    assert!(message.starts_with("Manifest is not valid JSON: "));
}

#[test]
fn empty_name_is_rejected() {
    let root = temp_root();
    let content = "{\"name\":\"\",\"version\":\"1.2.3\",\
\"entrypoint\":\"app.ts\"}";
    write_file(&root.path().join("manifest.json"), content.as_bytes());
    let message = manifest_message(root.path());
    assert_eq!(message, "Manifest name must not be empty");
}

#[test]
fn invalid_semver_is_rejected() {
    let root = temp_root();
    let content = "{\"name\":\"example-plugin\",\"version\":\"1.0\",\
\"entrypoint\":\"app.ts\"}";
    write_file(&root.path().join("manifest.json"), content.as_bytes());
    let message = manifest_message(root.path());
    assert_eq!(message, "Invalid SemVer version: 1.0");
}

#[test]
fn invalid_entrypoint_is_rejected() {
    let root = temp_root();
    let content = "{\"name\":\"example-plugin\",\"version\":\"1.2.3\",\
\"entrypoint\":\"../app.ts\"}";
    write_file(&root.path().join("manifest.json"), content.as_bytes());
    let message = manifest_message(root.path());
    assert_eq!(message, "Invalid entrypoint path: ../app.ts");
}

#[test]
fn name_with_parent_segment_is_rejected() {
    let message = name_message("../evil");
    assert_eq!(message, "Invalid manifest name: ../evil");
}

#[test]
fn name_with_separator_is_rejected() {
    let message = name_message("sub/evil");
    assert_eq!(message, "Invalid manifest name: sub/evil");
}

#[test]
fn absolute_name_is_rejected() {
    let message = name_message("/tmp/evil");
    assert_eq!(message, "Invalid manifest name: /tmp/evil");
}

#[test]
fn drive_letter_name_is_rejected() {
    let message = name_message("C:evil");
    assert_eq!(message, "Invalid manifest name: C:evil");
}

#[test]
fn byte_order_mark_is_rejected() {
    let root = temp_root();
    let mut content: Vec<u8> = Vec::new();
    content.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    content.extend_from_slice(VALID_MANIFEST.as_bytes());
    write_file(&root.path().join("manifest.json"), &content);
    let message = manifest_message(root.path());
    assert_eq!(message, "Manifest must not contain a byte order mark (BOM)");
}

#[test]
fn invalid_utf8_is_rejected() {
    let root = temp_root();
    let content: [u8; 2] = [0xFF, 0xFE];
    write_file(&root.path().join("manifest.json"), &content);
    let message = manifest_message(root.path());
    assert_eq!(message, "Manifest is not valid UTF-8");
}

#[test]
fn pack_does_not_create_archive_for_invalid_manifest() {
    let root = temp_root();
    let source = root.path().join("plugin");
    let dist = source.join("dist");
    create_dir(&dist);
    write_file(&dist.join("app.ts"), b"// app\n");
    let content = "{\"name\":\"\",\"version\":\"1.2.3\",\
\"entrypoint\":\"app.ts\"}";
    write_file(&source.join("manifest.json"), content.as_bytes());
    let target = root.path().join("target");
    let command = PackCommand::new(source, target.clone());
    let result = command.run();
    let error = match result {
        Ok(()) => panic!("expected packing to fail"),
        Err(value) => value,
    };
    let message = validation_message(error);
    assert_eq!(message, "Manifest name must not be empty");
    assert!(!target.exists() || !contains_archive(&target));
}

#[test]
fn pack_does_not_write_outside_target_for_escaping_name() {
    let root = temp_root();
    let source = root.path().join("plugin");
    let dist = source.join("dist");
    create_dir(&dist);
    write_file(&dist.join("app.ts"), b"// app\n");
    let content = "{\"name\":\"../escaped\",\"version\":\"1.2.3\",\
\"entrypoint\":\"app.ts\"}";
    write_file(&source.join("manifest.json"), content.as_bytes());
    let target = root.path().join("out").join("target");
    let command = PackCommand::new(source, target);
    let result = command.run();
    let error = match result {
        Ok(()) => panic!("expected packing to fail"),
        Err(value) => value,
    };
    let message = validation_message(error);
    assert_eq!(message, "Invalid manifest name: ../escaped");
    let escaped = root.path().join("out").join("escaped-1.2.3.van");
    assert!(!escaped.exists());
}

/// Create a temporary directory for one test.
fn temp_root() -> TempDir {
    let created = tempdir();
    return match created {
        Ok(value) => value,
        Err(error) => panic!("failed to create a temporary directory: {error}"),
    };
}

/// Write a file, failing the test when it cannot be written.
fn write_file(path: &Path, contents: &[u8]) {
    let written = fs::write(path, contents);
    match written {
        Ok(()) => {}
        Err(error) => panic!("failed to write {}: {error}", path.display()),
    }
}

/// Create a directory, failing the test when it cannot be created.
fn create_dir(path: &Path) {
    let created = fs::create_dir_all(path);
    match created {
        Ok(()) => {}
        Err(error) => panic!("failed to create {}: {error}", path.display()),
    }
}

/// Parse a manifest that must be accepted.
fn expect_manifest(source: &Path) -> Manifest {
    let result = Manifest::from_dir(source);
    return match result {
        Ok(value) => value,
        Err(error) => panic!("valid manifest was rejected: {error}"),
    };
}

/// Parse a manifest that must be rejected and return the message.
fn manifest_message(source: &Path) -> String {
    let result = Manifest::from_dir(source);
    let error = match result {
        Ok(_) => panic!("expected the manifest to be rejected"),
        Err(value) => value,
    };
    return validation_message(error);
}

/// Validate a manifest carrying `name` and return the error message.
fn name_message(name: &str) -> String {
    let root = temp_root();
    let content = format!(
        "{{\"name\":\"{name}\",\"version\":\"1.2.3\",\"entrypoint\":\"app.ts\"}}"
    );
    write_file(&root.path().join("manifest.json"), content.as_bytes());
    return manifest_message(root.path());
}

/// Extract the message of a validation error.
fn validation_message(error: Error) -> String {
    return match error {
        Error::ValidationError(message) => message,
        other => panic!("unexpected error: {other}"),
    };
}

/// Report whether a directory contains a `.van` archive.
fn contains_archive(directory: &Path) -> bool {
    let read = match fs::read_dir(directory) {
        Ok(value) => value,
        Err(error) => panic!("failed to read {}: {error}", directory.display()),
    };
    let mut read = read;
    loop {
        let next = read.next();
        let entry = match next {
            None => break,
            Some(Ok(value)) => value,
            Some(Err(error)) => {
                panic!("failed to read an entry: {error}");
            }
        };
        if entry.path().extension() == Some(OsStr::new("van")) {
            return true;
        }
    }
    return false;
}
