//! Integration test for the `pack` command.

use std::path::PathBuf;

use tempfile::tempdir;
use vantools::common::command::Command;
use vantools::pack::PackCommand;

/// Name of the archive the example plugin must produce.
const EXPECTED_ARCHIVE: &str = "test-plugin-0.1.0.van";

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
