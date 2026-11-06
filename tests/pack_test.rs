//! Integration tests for the `pack` command and the path validators.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tempfile::tempdir;
use vantools::common::Error;
use vantools::common::validation::{
    validate_archive_entry, validate_no_node_modules,
};
use vantools::pack::{PackCommand, PackReport};
use zip::ZipArchive;

/// Name of the archive the example plugin must produce.
const EXPECTED_ARCHIVE: &str = "test-plugin-0.1.0.van";

/// Manifest used by the temporary plugin sources in these tests.
const TEST_MANIFEST: &str =
    "{\"name\":\"bad-plugin\",\"version\":\"0.1.0\",\"entrypoint\":\"app.ts\"}";

/// Name of the archive built from [`TEST_MANIFEST`].
const BAD_ARCHIVE: &str = "bad-plugin-0.1.0.van";

/// Manifest with whitespace and an unknown member, used to prove that
/// the archived bytes are copied verbatim.
const RAW_MANIFEST: &str = "{\n  \"name\": \"raw-plugin\",\n  \"version\": \"2.0.0\",\n  \"entrypoint\": \"app.ts\",\n  \"extra\": \"ignored\"\n}";

#[test]
fn pack_rejects_missing_dist_directory() {
    let root = temp_root();
    let source = root.path().join("plugin");
    create_dir(&source);
    write_file(&source.join("manifest.json"), TEST_MANIFEST);
    let target = root.path().join("target");
    let error = pack_error(&source, &target);
    let expected =
        format!("Missing 'dist' directory in source: {}", source.display());
    assert_validation_message(error, &expected);
    assert!(!target.join(BAD_ARCHIVE).exists());
}

#[test]
fn pack_rejects_missing_entrypoint() {
    let root = temp_root();
    let source = root.path().join("plugin");
    let dist = source.join("dist");
    create_dir(&dist);
    write_file(&source.join("manifest.json"), TEST_MANIFEST);
    write_file(&dist.join("other.ts"), "// not the entrypoint\n");
    let target = root.path().join("target");
    let error = pack_error(&source, &target);
    let entrypoint = dist.join("app.ts");
    let expected =
        format!("Entrypoint file not found in dist: {}", entrypoint.display());
    assert_validation_message(error, &expected);
    assert!(!target.join(BAD_ARCHIVE).exists());
}

#[test]
fn pack_keeps_only_the_manifest_and_dist_contents() {
    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let nested = source.join("dist").join("src");
    create_dir(&nested);
    write_file(&nested.join("util.ts"), "// helper\n");
    write_file(&source.join(".gitignore"), "dist\n");
    let outside = source.join("outside");
    create_dir(&outside);
    write_file(&outside.join("secret.txt"), "not part of the plugin\n");
    let target = root.path().join("target");
    let report = pack_report(&source, &target);
    let archive = target.join(BAD_ARCHIVE);
    let mut names = archive_entry_names(&archive);
    names.sort();
    let mut expected: Vec<String> = vec![
        String::from("dist/app.ts"),
        String::from("dist/src/util.ts"),
        String::from("manifest.json"),
    ];
    expected.sort();
    assert_eq!(names, expected);
    assert_eq!(report.archive_path, target.join(BAD_ARCHIVE));
    assert_eq!(report.files_written, 3);
    assert_eq!(
        report.ignored_files,
        vec![PathBuf::from(".gitignore"), PathBuf::from("outside/secret.txt"),]
    );
}

#[test]
fn pack_collects_files_from_nested_dist_directories() {
    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let deep = source.join("dist").join("src").join("deep").join("nested");
    create_dir(&deep);
    write_file(&deep.join("leaf.ts"), "// leaf\n");
    let middle = source.join("dist").join("src");
    write_file(&middle.join("sibling.ts"), "// sibling\n");
    let target = root.path().join("target");
    let report = pack_report(&source, &target);
    let archive = target.join(BAD_ARCHIVE);
    let mut names = archive_entry_names(&archive);
    names.sort();
    let mut expected: Vec<String> = vec![
        String::from("dist/app.ts"),
        String::from("dist/src/deep/nested/leaf.ts"),
        String::from("dist/src/sibling.ts"),
        String::from("manifest.json"),
    ];
    expected.sort();
    assert_eq!(names, expected);
    assert_eq!(report.files_written, 4);
}

#[test]
fn pack_creates_van_archive() {
    let created = tempdir();
    let target_dir = match created {
        Ok(value) => value,
        Err(error) => panic!("failed to create a temporary directory: {error}"),
    };
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/test-plugin");
    let report = pack_report(&source, target_dir.path());
    let archive = target_dir.path().join(EXPECTED_ARCHIVE);
    assert_eq!(report.archive_path, archive);
    assert_eq!(report.files_written, 2);
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
fn pack_escapes_control_characters_in_source_file_names() {
    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let evil = source.join("dist").join("a\\b\u{1b}.ts");
    write_file(&evil, "// evil\n");
    let target = root.path().join("target");
    let error = pack_error(&source, &target);
    let message = match error {
        Error::ValidationError(value) => value,
        other => panic!("unexpected error: {other}"),
    };
    // The control character must be rendered as an escape, never emitted
    // raw, while printable characters such as the backslash stay as is.
    assert!(!message.contains('\u{1b}'));
    assert!(message.contains("\\u{1b}"));
    assert!(message.contains("a\\b"));
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

#[cfg(unix)]
#[test]
fn pack_rejects_symlinked_entrypoint() {
    use std::os::unix::fs::symlink;

    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    // A file outside `dist/` must not be reachable through the
    // entrypoint, which would otherwise be followed by the check.
    let outside = root.path().join("outside.ts");
    write_file(&outside, "// entrypoint outside dist\n");
    let entrypoint = source.join("dist").join("app.ts");
    remove_entrypoint_file(&entrypoint);
    let linked = symlink(&outside, &entrypoint);
    match linked {
        Ok(()) => {}
        Err(error) => panic!("failed to create a symlink: {error}"),
    }
    let target = root.path().join("target");
    let error = pack_error(&source, &target);
    let expected = format!(
        "Symlinks or non-regular files are not allowed: {}",
        entrypoint.display()
    );
    assert_validation_message(error, &expected);
    assert!(!target.join(BAD_ARCHIVE).exists());
}

#[cfg(unix)]
#[test]
fn pack_rejects_dangling_symlinked_entrypoint() {
    use std::os::unix::fs::symlink;

    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let entrypoint = source.join("dist").join("app.ts");
    remove_entrypoint_file(&entrypoint);
    let linked = symlink("missing.ts", &entrypoint);
    match linked {
        Ok(()) => {}
        Err(error) => panic!("failed to create a symlink: {error}"),
    }
    let target = root.path().join("target");
    let error = pack_error(&source, &target);
    let expected = format!(
        "Symlinks or non-regular files are not allowed: {}",
        entrypoint.display()
    );
    assert_validation_message(error, &expected);
    assert!(!target.join(BAD_ARCHIVE).exists());
}

#[cfg(unix)]
#[test]
fn pack_refuses_to_overwrite_symlinked_archive() {
    use std::os::unix::fs::symlink;

    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let target = root.path().join("target");
    create_dir(&target);
    let victim = root.path().join("victim.txt");
    write_file(&victim, "keep me\n");
    let archive = target.join(BAD_ARCHIVE);
    let linked = symlink(&victim, &archive);
    match linked {
        Ok(()) => {}
        Err(error) => panic!("failed to create a symlink: {error}"),
    }
    let error = pack_error(&source, &target);
    let expected = format!(
        "Refusing to overwrite a symlink or non-regular file: {}",
        archive.display()
    );
    assert_validation_message(error, &expected);
    let content = fs::read(&victim);
    let kept = match content {
        Ok(value) => value,
        Err(error) => panic!("failed to read the victim file: {error}"),
    };
    assert_eq!(kept.as_slice(), b"keep me\n".as_slice());
}

#[cfg(unix)]
#[test]
fn pack_refuses_symlinked_target_directory() {
    use std::os::unix::fs::symlink;

    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let destination = root.path().join("destination");
    create_dir(&destination);
    let target = root.path().join("target");
    let linked = symlink(&destination, &target);
    match linked {
        Ok(()) => {}
        Err(error) => panic!("failed to create a symlink: {error}"),
    }
    let error = pack_error(&source, &target);
    let expected = format!(
        "Symlinks or non-regular files are not allowed: {}",
        target.display()
    );
    assert_validation_message(error, &expected);
    // The archive must not be written through the link.
    assert!(!destination.join(BAD_ARCHIVE).exists());
}

#[cfg(unix)]
#[test]
fn pack_refuses_symlinked_source_directory() {
    use std::os::unix::fs::symlink;

    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let link_source = root.path().join("linked-plugin");
    let linked = symlink(&source, &link_source);
    match linked {
        Ok(()) => {}
        Err(error) => panic!("failed to create a symlink: {error}"),
    }
    let target = root.path().join("target");
    let error = pack_error(&link_source, &target);
    let expected = format!(
        "Symlinks or non-regular files are not allowed: {}",
        link_source.display()
    );
    assert_validation_message(error, &expected);
    assert!(!target.join(BAD_ARCHIVE).exists());
}

#[test]
fn pack_refuses_to_overwrite_directory_at_archive_path() {
    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let target = root.path().join("target");
    create_dir(&target);
    let archive = target.join(BAD_ARCHIVE);
    create_dir(&archive);
    let error = pack_error(&source, &target);
    let expected = format!(
        "Refusing to overwrite a symlink or non-regular file: {}",
        archive.display()
    );
    assert_validation_message(error, &expected);
}

#[test]
fn pack_overwrites_existing_regular_archive() {
    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let target = root.path().join("target");
    create_dir(&target);
    let archive = target.join(BAD_ARCHIVE);
    write_file(&archive, "stale\n");
    let report = pack_report(&source, &target);
    assert!(report.overwrote_existing);
    assert!(!report.target_created);
    let content = fs::read(&archive);
    let written = match content {
        Ok(value) => value,
        Err(error) => panic!("failed to read the archive: {error}"),
    };
    assert!(written.starts_with(b"PK".as_slice()));
    assert_no_temp_files(&target);
}

#[cfg(unix)]
#[test]
fn pack_failure_keeps_previous_archive_intact() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let target = root.path().join("target");
    create_dir(&target);
    let archive = target.join(BAD_ARCHIVE);
    write_file(&archive, "previous archive\n");
    // The second dist file is validated without being read, so it
    // passes collection and only fails when its content is streamed.
    let locked = source.join("dist").join("locked.ts");
    write_file(&locked, "// content that cannot be opened\n");
    let permitted = fs::Permissions::from_mode(0o000);
    let locked_permissions = fs::set_permissions(&locked, permitted);
    match locked_permissions {
        Ok(()) => {}
        Err(error) => panic!("failed to lock the source file: {error}"),
    }
    let error = pack_error(&source, &target);
    let message = match error {
        Error::Runtime(value) => value,
        other => panic!("unexpected error: {other}"),
    };
    assert!(
        message.contains(&format!("failed to read {}", locked.display())),
        "unexpected error: {message}"
    );
    let content = fs::read(&archive);
    let kept = match content {
        Ok(value) => value,
        Err(error) => panic!("failed to read the archive: {error}"),
    };
    assert_eq!(kept.as_slice(), b"previous archive\n".as_slice());
    assert_no_temp_files(&target);
}

#[test]
fn pack_clears_a_leftover_temporary_file() {
    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let target = root.path().join("target");
    create_dir(&target);
    let archive = target.join(BAD_ARCHIVE);
    let stale =
        target.join(format!("{BAD_ARCHIVE}.tmp-{}", std::process::id()));
    write_file(&stale, "stale leftover\n");
    let report = pack_report(&source, &target);
    assert!(!report.target_created);
    assert!(archive.is_file());
    assert!(!stale.exists());
    assert_no_temp_files(&target);
}

#[test]
fn pack_archives_the_validated_manifest_bytes() {
    let root = temp_root();
    let source = root.path().join("plugin");
    let dist = source.join("dist");
    create_dir(&dist);
    write_file(&dist.join("app.ts"), "// placeholder\n");
    write_file(&source.join("manifest.json"), RAW_MANIFEST);
    let target = root.path().join("target");
    let report = pack_report(&source, &target);
    let archive = target.join("raw-plugin-2.0.0.van");
    assert_eq!(report.archive_path, archive);
    let archived = read_archive_entry(&archive, "manifest.json");
    assert_eq!(archived.as_slice(), RAW_MANIFEST.as_bytes());
}

#[test]
fn pack_streams_source_files_in_chunks() {
    let root = temp_root();
    let source = root.path().join("plugin");
    create_source(&source);
    let payload = "chunked payload line\n".repeat(1024);
    write_file(&source.join("dist").join("app.ts"), &payload);
    let target = root.path().join("target");
    let report = pack_report(&source, &target);
    let archive = target.join(BAD_ARCHIVE);
    assert_eq!(report.files_written, 2);
    let archived = read_archive_entry(&archive, "dist/app.ts");
    assert_eq!(archived.as_slice(), payload.as_bytes());
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

/// Read one entry from a `.van` archive.
fn read_archive_entry(archive: &Path, entry_name: &str) -> Vec<u8> {
    let opened = fs::File::open(archive);
    let file = match opened {
        Ok(value) => value,
        Err(error) => panic!("failed to open {}: {error}", archive.display()),
    };
    let reader = ZipArchive::new(file);
    let mut archive_file = match reader {
        Ok(value) => value,
        Err(error) => panic!("failed to read {}: {error}", archive.display()),
    };
    let entry = archive_file.by_name(entry_name);
    let mut entry_file = match entry {
        Ok(value) => value,
        Err(error) => panic!("failed to read {entry_name}: {error}"),
    };
    let mut content: Vec<u8> = Vec::new();
    let read = entry_file.read_to_end(&mut content);
    match read {
        Ok(_) => {}
        Err(error) => panic!("failed to read {entry_name}: {error}"),
    }
    return content;
}

/// Collect the entry names of a `.van` archive in archive order.
fn archive_entry_names(archive: &Path) -> Vec<String> {
    let opened = fs::File::open(archive);
    let file = match opened {
        Ok(value) => value,
        Err(error) => panic!("failed to open {}: {error}", archive.display()),
    };
    let reader = ZipArchive::new(file);
    let mut archive_file = match reader {
        Ok(value) => value,
        Err(error) => panic!("failed to read {}: {error}", archive.display()),
    };
    let count = archive_file.len();
    let mut names: Vec<String> = Vec::new();
    for index in 0..count {
        let entry = archive_file.by_index(index);
        let entry_file = match entry {
            Ok(value) => value,
            Err(error) => panic!("failed to read entry {index}: {error}"),
        };
        let name = match entry_file.name() {
            Ok(value) => value,
            Err(error) => {
                panic!("failed to read the name of entry {index}: {error}")
            }
        };
        names.push(String::from(name));
    }
    return names;
}

/// Assert that the target directory holds no leftover `.tmp-` file.
fn assert_no_temp_files(target: &Path) {
    let listed = fs::read_dir(target);
    let reads = match listed {
        Ok(value) => value,
        Err(error) => {
            panic!("failed to read {}: {error}", target.display());
        }
    };
    for entry in reads {
        let entry = match entry {
            Ok(value) => value,
            Err(error) => {
                panic!(
                    "failed to read an entry of {}: {error}",
                    target.display()
                );
            }
        };
        let name = entry.file_name();
        let rendered = String::from(name.to_string_lossy());
        assert!(
            !rendered.contains(".tmp-"),
            "leftover temporary file: {rendered}"
        );
    }
}

/// Remove the regular entrypoint file so a test can replace it.
///
/// The helper is only for tests that put a symlink at the entrypoint
/// path, which `create_source` writes as a regular file.
fn remove_entrypoint_file(entrypoint: &Path) {
    let removed = fs::remove_file(entrypoint);
    match removed {
        Ok(()) => {}
        Err(error) => {
            panic!("failed to remove {}: {error}", entrypoint.display());
        }
    }
}

/// Run `pack` and return the report it must produce.
fn pack_report(source: &Path, target: &Path) -> PackReport {
    let command = PackCommand::new(source.to_path_buf(), target.to_path_buf());
    let result = command.run();
    return match result {
        Ok(value) => value,
        Err(error) => panic!("pack failed: {error}"),
    };
}

/// Run `pack` and return the error it must produce.
fn pack_error(source: &Path, target: &Path) -> Error {
    let command = PackCommand::new(source.to_path_buf(), target.to_path_buf());
    let result = command.run();
    return match result {
        Ok(_) => panic!("expected packing to be rejected"),
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
