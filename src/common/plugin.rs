//! Rules for the `.van` plugin layout in a source directory.
//!
//! The layout follows Section 4 of `docs/SPEC.md`: `manifest.json` sits
//! at the source root and the plugin payload lives in `dist/`. The
//! checks inspect paths without following symbolic links.

use std::path::Path;

use crate::common::Error;
use crate::common::fs::refuse_symlink;
use crate::common::validation::escape_path;

/// Name of the manifest file expected at the source root.
pub const MANIFEST_FILE_NAME: &str = "manifest.json";

/// Name of the directory that holds the plugin payload.
pub const DIST_DIRECTORY_NAME: &str = "dist";

/// Return an error unless `<source>/dist` is an existing directory.
///
/// The reported path is the source directory, because a missing `dist/`
/// means the source is not a packable plugin rather than that the user
/// passed a wrong `dist` path.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] when `dist_path` is missing or is
/// not a directory.
pub fn require_dist_directory(
    source: &Path,
    dist_path: &Path,
) -> Result<(), Error> {
    if dist_path.is_dir() {
        return Ok(());
    }
    return Err(Error::ValidationError(format!(
        "Missing 'dist' directory in source: {}",
        escape_path(source)
    )));
}

/// Return an error unless `<dist>/<entrypoint>` is an existing file.
///
/// The `entrypoint` is validated as a relative archive path when the
/// manifest is read, so joining it onto `dist_path` cannot escape the
/// `dist` directory.
///
/// A symlink at the entrypoint path is refused instead of followed, so
/// the dedicated check enforces the no-symlink policy itself instead of
/// relying on the later `dist/` walk. The link is rejected with the
/// same diagnostic [`crate::common::validation::validate_file_type`]
/// uses for `dist/` and its contents. A missing or non-regular path,
/// such as a directory or a FIFO, is reported as missing because the
/// entrypoint must be a regular file.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] for a symlinked, non-regular, or
/// missing entrypoint, and [`Error::Runtime`] when the path cannot be
/// inspected.
pub fn require_entrypoint(
    dist_path: &Path,
    entrypoint: &str,
) -> Result<(), Error> {
    let entrypoint_path = dist_path.join(entrypoint);
    refuse_symlink(&entrypoint_path)?;
    if entrypoint_path.is_file() {
        return Ok(());
    }
    return Err(Error::ValidationError(format!(
        "Entrypoint file not found in dist: {}",
        escape_path(&entrypoint_path)
    )));
}
