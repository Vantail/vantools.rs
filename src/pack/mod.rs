//! Implementation of the `pack` command.
//!
//! This module holds the `PackCommand` type and the source validation it
//! drives. A run writes only the archive and returns a [`PackReport`];
//! the low-level archive writing lives in the `writer` submodule and
//! argument parsing lives in the `van` binary.

use std::fs;
use std::path::{Path, PathBuf};

use crate::common::Error;
use crate::common::cli::CliCommand;
use crate::common::fs::{refuse_symlink, require_directory, require_file};
use crate::common::manifest::Manifest;
use crate::common::plugin::{
    DIST_DIRECTORY_NAME, MANIFEST_FILE_NAME, require_dist_directory,
    require_entrypoint,
};
use crate::common::validation::{
    escape_path, validate_archive_entry, validate_file_type,
    validate_no_node_modules,
};
use crate::common::walk::{WalkEntry, walk_tree};

mod writer;

use writer::write_archive;

/// Extension of the archives this command produces.
const ARCHIVE_EXTENSION: &str = "van";

/// The `pack` command.
///
/// Reads a plugin source directory and writes a `<name>-<version>.van`
/// archive into the target directory. An existing archive with the same
/// name is overwritten; an existing symlink or other non-regular entry
/// is refused so the archive cannot be redirected outside the target
/// directory. A successful run returns a [`PackReport`] with the archive
/// path, the files written, the source files the layout excluded, and
/// whether the target directory was created or an archive was replaced.
///
/// The source and target directories themselves are inspected without
/// following links, so a symlinked directory is refused rather than
/// resolved, matching the no-symlink policy applied to every other path
/// the command touches.
///
/// The archive layout follows Section 4 of `docs/SPEC.md`: the
/// validated `manifest.json` sits at the archive root and only the
/// contents of `dist/` are added, each under the `dist/` prefix. Other
/// entries of the source directory are never read into the archive.
#[non_exhaustive]
pub struct PackCommand {
    /// The plugin source directory holding `manifest.json` and `dist/`.
    pub source: PathBuf,
    /// The directory that receives the generated archive.
    pub target: PathBuf,
}

/// Everything a completed pack run produced.
#[non_exhaustive]
pub struct PackReport {
    /// Path of the archive that was written.
    pub archive_path: PathBuf,
    /// Number of files written to the archive, including the manifest.
    pub files_written: usize,
    /// Source files the archive layout excluded, in sorted order.
    pub ignored_files: Vec<PathBuf>,
    /// Whether the target directory had to be created.
    pub target_created: bool,
    /// Whether an existing archive was replaced.
    pub overwrote_existing: bool,
}

/// A validated source file scheduled for the archive.
struct ArchiveEntry {
    /// Name of the entry inside the archive.
    archive_name: String,
    /// Path of the file on disk.
    source_path: PathBuf,
}

/// Everything a validated source directory contributes to the archive.
struct CollectedPack {
    /// The parsed and validated `manifest.json`.
    manifest: Manifest,
    /// The exact manifest bytes that passed validation.
    manifest_bytes: Vec<u8>,
    /// The files scheduled for the archive, in walk order.
    entries: Vec<ArchiveEntry>,
    /// The source files the archive layout excludes.
    ignored: Vec<PathBuf>,
}

impl PackCommand {
    /// Creates a `pack` command for the given source and target.
    pub fn new(source: PathBuf, target: PathBuf) -> Self {
        return Self {
            source,
            target,
        };
    }

    /// Run the command and return what it produced.
    ///
    /// The command writes only the archive; it prints nothing, so the
    /// caller decides how to report the result.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when validation or the archive write fails.
    pub fn run(&self) -> Result<PackReport, Error> {
        let dist_path = self.validate_layout()?;
        let collected = self.collect_pack(&dist_path)?;
        return self.write_collected(collected);
    }

    /// Build the archive path from the manifest name and version.
    fn archive_path(&self, manifest: &Manifest) -> PathBuf {
        let archive_name = format!(
            "{}-{}.{ARCHIVE_EXTENSION}",
            manifest.name, manifest.version
        );
        return self.target.join(archive_name);
    }

    /// Create the target directory when it does not exist yet.
    ///
    /// A symlink at the target path is refused instead of followed, so
    /// the archive cannot land in a directory other than the one the
    /// command named. `create_dir_all` resolves links, so the created
    /// entry is inspected again and a link planted in between is refused
    /// too. The returned flag records whether the directory had to be
    /// created, so the caller can name its path in the report.
    fn ensure_target(&self) -> Result<bool, Error> {
        refuse_symlink(&self.target)?;
        if self.target.is_dir() {
            return Ok(false);
        }
        let created = fs::create_dir_all(&self.target);
        match created {
            Ok(()) => {}
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to create target directory {}: {error}",
                    escape_path(&self.target)
                )));
            }
        }
        require_created_directory(&self.target)?;
        return Ok(true);
    }

    /// Validate the source layout and return the `dist` directory.
    ///
    /// This covers the first phase of a pack run: the source directory,
    /// its `manifest.json`, and its `dist/` directory must all exist,
    /// each as a regular directory or file rather than a symlink. The
    /// checks run in this fixed order so the first failure is always the
    /// same error, with the same type and path, as before.
    fn validate_layout(&self) -> Result<PathBuf, Error> {
        let manifest_path = self.source.join(MANIFEST_FILE_NAME);
        let dist_path = self.source.join(DIST_DIRECTORY_NAME);
        require_directory(&self.source, "source directory")?;
        require_file(&manifest_path, MANIFEST_FILE_NAME)?;
        validate_file_type(&manifest_path)?;
        require_dist_directory(&self.source, &dist_path)?;
        validate_file_type(&dist_path)?;
        return Ok(dist_path);
    }

    /// Load the manifest and collect everything an archive will contain.
    ///
    /// The manifest is read and validated, the entrypoint is checked
    /// against `dist`, then the walk collects the archive entries and
    /// the ignored paths into a [`CollectedPack`].
    fn collect_pack(&self, dist_path: &Path) -> Result<CollectedPack, Error> {
        let loaded = Manifest::from_dir_with_bytes(&self.source)?;
        let (manifest, manifest_bytes) = loaded;
        require_entrypoint(dist_path, &manifest.entrypoint)?;
        let mut entries: Vec<ArchiveEntry> = Vec::new();
        collect_entries(dist_path, DIST_DIRECTORY_NAME, &mut entries)?;
        let mut ignored: Vec<PathBuf> = Vec::new();
        collect_ignored(&self.source, &mut ignored)?;
        return Ok(CollectedPack {
            manifest,
            manifest_bytes,
            entries,
            ignored,
        });
    }

    /// Prepare the target and write the archive.
    ///
    /// This covers the final phase of a pack run: the target directory
    /// is created, an existing archive is recorded as replaced, the
    /// archive is written, and the ignored paths are sorted for the
    /// report.
    fn write_collected(
        &self,
        collected: CollectedPack,
    ) -> Result<PackReport, Error> {
        let CollectedPack {
            manifest,
            manifest_bytes,
            entries,
            ignored: mut source_ignored,
        } = collected;
        let target_created = self.ensure_target()?;
        let archive_path = self.archive_path(&manifest);
        let overwrote_existing = archive_path.exists();
        write_archive(&archive_path, &manifest_bytes, &entries)?;
        source_ignored.sort();
        let files_written = entries.len() + 1;
        return Ok(PackReport {
            archive_path,
            files_written,
            ignored_files: source_ignored,
            target_created,
            overwrote_existing,
        });
    }
}

/// Refuse a target that is not a real directory after it was created.
///
/// `fs::create_dir_all` resolves symbolic links, so a link planted after
/// [`refuse_symlink`] inspected the path is followed and the creation
/// succeeds through it. Inspecting the entry itself again, without
/// following links, refuses that link instead of letting the archive be
/// written through it.
///
/// # Errors
///
/// Returns [`Error::ValidationError`] for a symbolic link or any other
/// non-directory entry, and [`Error::Runtime`] when the path cannot be
/// inspected.
fn require_created_directory(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path);
    return match metadata {
        Ok(value) => {
            if value.file_type().is_dir() {
                return Ok(());
            }
            Err(Error::ValidationError(format!(
                "Symlinks or non-regular files are not allowed: {}",
                escape_path(path)
            )))
        }
        Err(error) => Err(Error::Runtime(format!(
            "failed to inspect {}: {error}",
            escape_path(path)
        ))),
    };
}

impl CliCommand for PackCommand {
    fn name(&self) -> &'static str {
        return "pack";
    }

    fn usage(&self) -> &'static str {
        return "pack -s <source> -t <target>";
    }

    fn help(&self) -> &'static str {
        return "Usage: pack -s <source> -t <target>\n\nOptions:\n  -s, --source <path>  The source directory containing manifest.json and dist/\n  -t, --target <path>  The target directory to place the .van archive\n  -h, --help           Print this help message\n  -v, --version        Print version information";
    }
}

/// Collect the files of `source` that the archive layout excludes.
///
/// The manifest at the source root and the `dist/` directory belong to
/// the archive, so every other file is reported to the user.
fn collect_ignored(
    source: &Path,
    ignored: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    let mut visit = |entry: &WalkEntry| -> Result<bool, Error> {
        if entry.file_type.is_dir() {
            let relative = entry.relative.as_path();
            return Ok(relative != Path::new(DIST_DIRECTORY_NAME));
        }
        if entry.relative.as_path() == Path::new(MANIFEST_FILE_NAME) {
            return Ok(false);
        }
        ignored.push(entry.relative.clone());
        return Ok(false);
    };
    return walk_tree(source, &mut visit);
}

/// Validate and collect every file below `directory` under `prefix`.
///
/// The walk itself lives in [`walk_tree`], which keeps pending
/// directories on the heap, so the nesting depth of the source tree
/// cannot exhaust the stack.
fn collect_entries(
    directory: &Path,
    prefix: &str,
    entries: &mut Vec<ArchiveEntry>,
) -> Result<(), Error> {
    let mut visit = |entry: &WalkEntry| -> Result<bool, Error> {
        validate_file_type(&entry.path)?;
        let archive_name = archive_name_of(prefix, &entry.relative)?;
        validate_no_node_modules(Path::new(&archive_name))?;
        validate_archive_entry(&archive_name)?;
        if entry.file_type.is_dir() {
            return Ok(true);
        }
        entries.push(ArchiveEntry {
            archive_name,
            source_path: entry.path.clone(),
        });
        return Ok(false);
    };
    return walk_tree(directory, &mut visit);
}

/// Build an archive name from a prefix and a path relative to it.
///
/// Archive names always use `/`, and `validate_archive_entry` rejects
/// any backslash, so a name never depends on the host separator.
fn archive_name_of(prefix: &str, relative: &Path) -> Result<String, Error> {
    let mut name = String::from(prefix);
    for component in relative.components() {
        let part = match component.as_os_str().to_str() {
            Some(value) => value,
            None => {
                return Err(Error::Runtime(format!(
                    "file name is not valid Unicode: {}",
                    escape_path(relative)
                )));
            }
        };
        name.push('/');
        name.push_str(part);
    }
    return Ok(name);
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;
    use tempfile::tempdir;

    use super::require_created_directory;
    use crate::common::Error;
    use crate::common::validation::escape_path;

    /// Create a temporary directory for one test.
    fn temp_root() -> TempDir {
        let created = tempdir();
        return match created {
            Ok(value) => value,
            Err(error) => {
                panic!("failed to create a temporary directory: {error}");
            }
        };
    }

    /// Assert that a path was refused as a validation error.
    fn assert_refused(error: Result<(), Error>, expected: &str) {
        match error {
            Ok(()) => panic!("expected the path to be refused"),
            Err(Error::ValidationError(message)) => {
                assert_eq!(message, expected);
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn created_directory_accepts_a_real_directory() {
        let root = temp_root();
        let directory = root.path().join("target");
        let created = fs::create_dir(&directory);
        match created {
            Ok(()) => {}
            Err(error) => {
                panic!("failed to create {}: {error}", directory.display());
            }
        }
        assert!(require_created_directory(&directory).is_ok());
    }

    #[test]
    fn created_directory_refuses_a_regular_file() {
        let root = temp_root();
        let file = root.path().join("target");
        let written = fs::write(&file, "not a directory\n");
        match written {
            Ok(()) => {}
            Err(error) => {
                panic!("failed to write {}: {error}", file.display());
            }
        }
        let expected = format!(
            "Symlinks or non-regular files are not allowed: {}",
            escape_path(&file)
        );
        assert_refused(require_created_directory(&file), &expected);
    }

    #[test]
    fn created_directory_reports_a_missing_path() {
        let root = temp_root();
        let missing = root.path().join("target");
        match require_created_directory(&missing) {
            Ok(()) => panic!("expected the missing path to be refused"),
            Err(Error::Runtime(message)) => {
                let rendered = escape_path(&missing);
                let expected = format!("failed to inspect {rendered}");
                assert!(message.contains(&expected), "unexpected: {message}");
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn created_directory_refuses_a_symlink() {
        use std::os::unix::fs::symlink;

        let root = temp_root();
        let real = root.path().join("real");
        let created = fs::create_dir(&real);
        match created {
            Ok(()) => {}
            Err(error) => {
                panic!("failed to create {}: {error}", real.display());
            }
        }
        let link = root.path().join("target");
        let linked = symlink(&real, &link);
        match linked {
            Ok(()) => {}
            Err(error) => {
                panic!("failed to create {}: {error}", link.display());
            }
        }
        let expected = format!(
            "Symlinks or non-regular files are not allowed: {}",
            escape_path(&link)
        );
        assert_refused(require_created_directory(&link), &expected);
    }
}
