//! End-to-end test for the complete `.van` plugin lifecycle.

use std::fs;
use std::path::Path;

use tempfile::TempDir;
use tempfile::tempdir;
use vantools::pack::PackCommand;
use vantools::unpack::UnpackCommand;
use vantools::verify::VerifyCommand;

/// Plugin name used by the round-trip test.
const PLUGIN_NAME: &str = "test-plugin";

/// Archive file name derived from the manifest name and version.
const ARCHIVE_NAME: &str = "test-plugin-0.1.0.van";

/// Directory the archive extracts into.
const PLUGIN_FOLDER: &str = "test-plugin-0.1.0";

/// Manifest written to the source directory.
const SAMPLE_MANIFEST: &str = "{\"name\":\"test-plugin\",\"version\":\"0.1.0\",\"entrypoint\":\"app.ts\"}";

/// Entrypoint written below `dist/`.
const APP_SOURCE: &str = "export function run(): void {}\n";

/// Nested helper written below `dist/subdir/`.
const UTIL_SOURCE: &str = "export const answer = 42;\n";

#[test]
fn pack_verify_unpack_roundtrip() {
    let workspace = temp_root();
    let source = workspace.path().join("source");
    write_source(&source);
    let archives = workspace.path().join("archives");
    let pack = PackCommand::new(source.clone(), archives.clone());
    let packed = pack.run();
    match packed {
        Ok(_) => {}
        Err(error) => panic!("pack failed: {error}"),
    }
    let archive = archives.join(ARCHIVE_NAME);
    assert!(
        archive.is_file(),
        "archive was not written: {}",
        archive.display()
    );
    let verify = VerifyCommand::new(archive.clone());
    let verified = verify.run();
    let verify_report = match verified {
        Ok(value) => value,
        Err(error) => panic!("verify failed: {error}"),
    };
    assert_eq!(verify_report.manifest.name, PLUGIN_NAME);
    assert_eq!(verify_report.entry_names.len(), 3);
    let out = workspace.path().join("out");
    let unpack = UnpackCommand::new(archive, out.clone());
    let unpacked = unpack.run();
    let unpack_report = match unpacked {
        Ok(value) => value,
        Err(error) => panic!("unpack failed: {error}"),
    };
    assert_eq!(unpack_report.files_written, 3);
    let destination = out.join(PLUGIN_FOLDER);
    assert_file_matches(
        &destination.join("manifest.json"),
        &source.join("manifest.json"),
    );
    assert_file_matches(
        &destination.join("dist").join("app.ts"),
        &source.join("dist").join("app.ts"),
    );
    assert_file_matches(
        &destination.join("dist").join("subdir").join("util.ts"),
        &source.join("dist").join("subdir").join("util.ts"),
    );
}

/// Create the plugin source tree used by the round trip.
fn write_source(source: &Path) {
    write_file(&source.join("manifest.json"), SAMPLE_MANIFEST);
    write_file(&source.join("dist").join("app.ts"), APP_SOURCE);
    write_file(
        &source.join("dist").join("subdir").join("util.ts"),
        UTIL_SOURCE,
    );
}

/// Write `contents` to `path`, creating missing parent directories.
fn write_file(path: &Path, contents: &str) {
    let parent = match path.parent() {
        Some(value) => value,
        None => panic!("path has no parent: {}", path.display()),
    };
    let created = fs::create_dir_all(parent);
    match created {
        Ok(()) => {}
        Err(error) => {
            panic!("failed to create {}: {error}", parent.display());
        }
    }
    let written = fs::write(path, contents);
    match written {
        Ok(()) => {}
        Err(error) => panic!("failed to write {}: {error}", path.display()),
    }
}

/// Assert that `actual` holds exactly the bytes of `expected`.
fn assert_file_matches(actual: &Path, expected: &Path) {
    assert!(
        actual.is_file(),
        "extracted file is missing: {}",
        actual.display()
    );
    let actual_bytes = fs::read(actual);
    let actual_content = match actual_bytes {
        Ok(value) => value,
        Err(error) => {
            panic!("failed to read {}: {error}", actual.display());
        }
    };
    let expected_bytes = fs::read(expected);
    let expected_content = match expected_bytes {
        Ok(value) => value,
        Err(error) => {
            panic!("failed to read {}: {error}", expected.display());
        }
    };
    assert_eq!(
        actual_content,
        expected_content,
        "extracted file differs from {}",
        expected.display()
    );
}

/// Create a temporary workspace for the round trip.
fn temp_root() -> TempDir {
    let created = tempdir();
    return match created {
        Ok(value) => value,
        Err(error) => panic!("failed to create a temporary directory: {error}"),
    };
}
