//! Integration tests for the `unpack` command.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;

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

/// Directory the sample archive extracts into, derived from its name and
/// version.
const SAMPLE_FOLDER: &str = "sample-plugin-0.1.0";

/// Manifest used by the archives that are expected to be valid.
const SAMPLE_MANIFEST: &str = "{\"name\":\"sample-plugin\",\"version\":\"0.1.0\",\"entrypoint\":\"app.ts\"}";

/// Size of the entry that keeps extraction running while the archive
/// path is replaced.
const SLOW_ENTRY_BYTES: usize = 32 * 1024 * 1024;

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
fn unpack_extracts_into_a_name_version_directory() {
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
    let destination = target.join(SAMPLE_FOLDER);
    assert_eq!(report.archive, archive);
    assert_eq!(report.target, destination);
    assert_eq!(report.manifest.entrypoint, "app.ts");
    assert_eq!(report.files_written, 2);
    // The base target is created when it is missing.
    assert!(target.is_dir());
    assert!(destination.is_dir());
    assert_file_content(&destination.join("manifest.json"), SAMPLE_MANIFEST);
    assert_file_content(
        &destination.join("dist").join("app.ts"),
        "// entrypoint content\n",
    );
    assert_no_temp_entries(&target);
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
    let destination = target.join(SAMPLE_FOLDER);
    assert_eq!(report.target, destination);
    assert_eq!(report.files_written, 3);
    assert_file_content(
        &destination.join("dist").join("src").join("util.ts"),
        "// helper\n",
    );
    assert_no_temp_entries(&target);
}

#[test]
fn unpack_rejects_an_existing_plugin_directory() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(
        &archive,
        &[("manifest.json", SAMPLE_MANIFEST), ("dist/app.ts", "// content\n")],
    );
    let target = root.path().join("out");
    let destination = target.join(SAMPLE_FOLDER);
    create_dir(&destination);
    let sentinel = destination.join("keep.txt");
    write_file(&sentinel, "user data\n");
    let error = unpack_error(&archive, &target);
    let expected = format!(
        "Target directory already exists: {}",
        escape_path(&destination)
    );
    assert_validation_message(error, &expected);
    assert_file_content(&sentinel, "user data\n");
    assert_no_temp_entries(&target);
}

#[test]
fn unpack_refuses_an_existing_temporary_directory() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(
        &archive,
        &[("manifest.json", SAMPLE_MANIFEST), ("dist/app.ts", "// content\n")],
    );
    let target = root.path().join("out");
    create_dir(&target);
    let destination = target.join(SAMPLE_FOLDER);
    let leftover = temporary_path(&destination);
    create_dir(&leftover);
    let stale = leftover.join("stale.txt");
    write_file(&stale, "stale leftover\n");
    let error = unpack_error(&archive, &target);
    let expected = format!(
        "Temporary directory already exists: {}",
        escape_path(&leftover)
    );
    assert_validation_message(error, &expected);
    assert_file_content(&stale, "stale leftover\n");
    assert!(!destination.exists());
}

#[cfg(unix)]
#[test]
fn unpack_refuses_a_symlinked_temporary_directory() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(
        &archive,
        &[("manifest.json", SAMPLE_MANIFEST), ("dist/app.ts", "// content\n")],
    );
    let target = root.path().join("out");
    create_dir(&target);
    let destination = target.join(SAMPLE_FOLDER);
    let leftover = temporary_path(&destination);
    let elsewhere = root.path().join("elsewhere");
    create_dir(&elsewhere);
    let linked = std::os::unix::fs::symlink(&elsewhere, &leftover);
    match linked {
        Ok(()) => {}
        Err(error) => {
            panic!("failed to create {}: {error}", leftover.display());
        }
    }
    let error = unpack_error(&archive, &target);
    let expected = format!(
        "Temporary directory already exists: {}",
        escape_path(&leftover)
    );
    assert_validation_message(error, &expected);
    assert!(!elsewhere.join("manifest.json").exists());
    assert!(!destination.exists());
}

#[cfg(unix)]
#[test]
fn unpack_rejects_a_symlinked_target() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(
        &archive,
        &[("manifest.json", SAMPLE_MANIFEST), ("dist/app.ts", "// content\n")],
    );
    let real_target = root.path().join("real");
    create_dir(&real_target);
    let target = root.path().join("out");
    let linked = std::os::unix::fs::symlink(&real_target, &target);
    match linked {
        Ok(()) => {}
        Err(error) => {
            panic!("failed to create {}: {error}", target.display());
        }
    }
    let error = unpack_error(&archive, &target);
    let expected = format!(
        "Target is a symlink or not a directory: {}",
        escape_path(&target)
    );
    assert_validation_message(error, &expected);
    assert!(!real_target.join(SAMPLE_FOLDER).exists());
    assert_no_temp_entries(&real_target);
}

#[cfg(unix)]
#[test]
fn extraction_reads_the_handle_that_was_validated() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    let mut content = String::from("// validated\n");
    content.push_str(&"a".repeat(SLOW_ENTRY_BYTES));
    write_archive(
        &archive,
        &[("manifest.json", SAMPLE_MANIFEST), ("dist/app.ts", &content)],
    );
    let replacement = root.path().join("replacement.van");
    write_archive(
        &replacement,
        &[
            ("manifest.json", SAMPLE_MANIFEST),
            ("dist/app.ts", "// swapped\n"),
            ("dist/../escaped.ts", "// escaped\n"),
        ],
    );
    let target = root.path().join("out");
    let destination = target.join(SAMPLE_FOLDER);
    let worker_archive = archive.clone();
    let worker_target = target.clone();
    let worker = thread::spawn(move || {
        let command = UnpackCommand::new(worker_archive, worker_target);
        return command.run();
    });
    let swapped = replace_during_extraction(
        &worker,
        &archive,
        &replacement,
        &destination,
    );
    let joined = worker.join();
    let result = match joined {
        Ok(value) => value,
        Err(_) => panic!("the unpack thread panicked"),
    };
    let report = match result {
        Ok(value) => value,
        Err(error) => panic!("unpack failed: {error}"),
    };
    assert!(swapped, "the archive was not replaced before extraction finished");
    assert_eq!(report.files_written, 2);
    assert_file_content(&destination.join("dist").join("app.ts"), &content);
    assert!(!destination.join("escaped.ts").exists());
}

/// Replace `archive` while its extraction is still running.
///
/// The temporary extraction directory appears after validation and
/// stays until the finished directory is renamed into place, so it
/// gives the moment to swap the path. Returns whether the replacement
/// happened before the destination existed.
#[cfg(unix)]
fn replace_during_extraction(
    worker: &thread::JoinHandle<Result<UnpackReport, Error>>,
    archive: &Path,
    replacement: &Path,
    destination: &Path,
) -> bool {
    let temporary = temporary_path(destination);
    loop {
        if destination.exists() {
            return false;
        }
        if temporary.exists() {
            let replaced = fs::rename(replacement, archive);
            match replaced {
                Ok(()) => return !destination.exists(),
                Err(error) => {
                    panic!("failed to replace {}: {error}", archive.display());
                }
            }
        }
        if worker.is_finished() {
            return false;
        }
        thread::yield_now();
    }
}

#[test]
fn unpack_rejects_a_non_directory_target() {
    let root = temp_root();
    let archive = root.path().join(SAMPLE_ARCHIVE);
    write_archive(
        &archive,
        &[("manifest.json", SAMPLE_MANIFEST), ("dist/app.ts", "// content\n")],
    );
    let target = root.path().join("out");
    write_file(&target, "not a directory\n");
    let error = unpack_error(&archive, &target);
    let expected = format!(
        "Target is a symlink or not a directory: {}",
        escape_path(&target)
    );
    assert_validation_message(error, &expected);
    assert_file_content(&target, "not a directory\n");
}

/// Create a temporary directory for one test.
fn temp_root() -> TempDir {
    let created = tempdir();
    return match created {
        Ok(value) => value,
        Err(error) => panic!("failed to create a temporary directory: {error}"),
    };
}

/// Derive the temporary directory a run uses for `destination`.
///
/// The writer keeps the process id in the name, so a test can place a
/// pre-existing entry at exactly that path.
fn temporary_path(destination: &Path) -> PathBuf {
    let name = match destination.file_name() {
        Some(value) => value.to_owned(),
        None => {
            panic!("destination has no file name: {}", destination.display())
        }
    };
    let mut temp_name = name;
    temp_name.push(format!(".tmp-{}", std::process::id()));
    return destination.with_file_name(temp_name);
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
