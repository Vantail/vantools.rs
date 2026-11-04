//! Implementation of the `pack` command.

use std::fs::{self, File};
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};

use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::common::Error;
use crate::common::cli::CliCommand;
use crate::common::command::Command;
use crate::common::manifest::Manifest;
use crate::common::validation::{
    validate_archive_entry, validate_file_type, validate_no_node_modules,
};

/// Name of the manifest file expected at the source root.
const MANIFEST_FILE_NAME: &str = "manifest.json";

/// Name of the directory that holds the plugin payload.
const DIST_DIRECTORY_NAME: &str = "dist";

/// Extension of the archives this command produces.
const ARCHIVE_EXTENSION: &str = "van";

/// Size of the buffer used to copy one source file into the archive.
const COPY_BUFFER_BYTES: usize = 8 * 1024;

/// The `pack` command.
///
/// Reads a plugin source directory and writes a `<name>-<version>.van`
/// archive into the target directory. An existing regular archive with
/// the same name is overwritten; an existing symlink or other
/// non-regular entry is refused so the archive cannot be redirected
/// outside the target directory.
#[non_exhaustive]
pub struct PackCommand {
    /// The plugin source directory holding `manifest.json` and `dist/`.
    pub source: PathBuf,
    /// The directory that receives the generated archive.
    pub target: PathBuf,
}

/// A validated source file scheduled for the archive.
struct ArchiveEntry {
    /// Name of the entry inside the archive.
    archive_name: String,
    /// Path of the file on disk.
    source_path: PathBuf,
}

impl Command for PackCommand {
    fn run(&self) -> Result<(), Error> {
        let manifest_path = self.source.join(MANIFEST_FILE_NAME);
        let dist_path = self.source.join(DIST_DIRECTORY_NAME);
        require_directory(&self.source, "source directory")?;
        require_file(&manifest_path, MANIFEST_FILE_NAME)?;
        require_directory(&dist_path, DIST_DIRECTORY_NAME)?;
        validate_file_type(&manifest_path)?;
        validate_file_type(&dist_path)?;
        let mut entries: Vec<ArchiveEntry> = Vec::new();
        collect_entries(&dist_path, DIST_DIRECTORY_NAME, &mut entries)?;
        self.ensure_target()?;
        let loaded = Manifest::from_dir_with_bytes(&self.source)?;
        let (manifest, manifest_bytes) = loaded;
        let archive_path = self.archive_path(&manifest);
        let file = create_archive_file(&archive_path)?;
        let mut writer = ZipWriter::new(file);
        let options = default_file_options();
        add_manifest(&mut writer, &manifest_bytes, options)?;
        write_entries(&mut writer, &entries, options)?;
        let finished = writer.finish();
        return match finished {
            Ok(_) => Ok(()),
            Err(error) => Err(Error::Runtime(format!(
                "failed to finish {}: {error}",
                archive_path.display()
            ))),
        };
    }
}

impl PackCommand {
    /// Creates a `pack` command for the given source and target.
    pub fn new(source: PathBuf, target: PathBuf) -> Self {
        return Self {
            source,
            target,
        };
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
    fn ensure_target(&self) -> Result<(), Error> {
        if self.target.is_dir() {
            return Ok(());
        }
        let created = fs::create_dir_all(&self.target);
        return match created {
            Ok(()) => notice(&format!(
                "created target directory: {}",
                self.target.display()
            )),
            Err(error) => Err(Error::Runtime(format!(
                "failed to create target directory {}: {error}",
                self.target.display()
            ))),
        };
    }
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

/// Options applied to every archive entry: `deflate` compression.
fn default_file_options() -> SimpleFileOptions {
    return SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated);
}

/// Return an error unless `path` is an existing directory.
fn require_directory(path: &Path, description: &str) -> Result<(), Error> {
    if path.is_dir() {
        return Ok(());
    }
    return Err(Error::Runtime(format!(
        "{description} not found: {}",
        path.display()
    )));
}

/// Return an error unless `path` is an existing regular file.
fn require_file(path: &Path, description: &str) -> Result<(), Error> {
    if path.is_file() {
        return Ok(());
    }
    return Err(Error::Runtime(format!(
        "{description} not found: {}",
        path.display()
    )));
}

/// Print a notice about a successful side effect to stdout.
fn notice(message: &str) -> Result<(), Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let written = writeln!(handle, "{message}");
    return match written {
        Ok(()) => Ok(()),
        Err(error) => {
            Err(Error::Runtime(format!("failed to write output: {error}")))
        }
    };
}

/// Create the archive file without following a symlink.
///
/// A missing path is created, and an existing regular file is
/// truncated so that re-packing replaces the previous archive. Any
/// other existing entry, such as a symlink or a directory, is rejected
/// so a planted link cannot redirect the archive outside the target
/// directory.
fn create_archive_file(path: &Path) -> Result<File, Error> {
    let metadata = fs::symlink_metadata(path);
    match metadata {
        Ok(value) => {
            if !value.file_type().is_file() {
                return Err(Error::ValidationError(format!(
                    "Refusing to overwrite a symlink or non-regular file: {}",
                    path.display()
                )));
            }
        }
        Err(error) => {
            if error.kind() != io::ErrorKind::NotFound {
                return Err(Error::Runtime(format!(
                    "failed to inspect {}: {error}",
                    path.display()
                )));
            }
        }
    }
    let created = File::create(path);
    return match created {
        Ok(value) => Ok(value),
        Err(error) => Err(Error::Runtime(format!(
            "failed to create {}: {error}",
            path.display()
        ))),
    };
}

/// Add the validated manifest bytes to the archive root.
fn add_manifest(
    writer: &mut ZipWriter<File>,
    manifest_bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<(), Error> {
    return add_file(writer, MANIFEST_FILE_NAME, manifest_bytes, options);
}

/// Start one archive entry under `archive_name`.
fn start_entry(
    writer: &mut ZipWriter<File>,
    archive_name: &str,
    options: SimpleFileOptions,
) -> Result<(), Error> {
    let started = writer.start_file(archive_name, options);
    return match started {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to add {archive_name}: {error}"
        ))),
    };
}

/// Add `content` to the archive under `archive_name`.
fn add_file(
    writer: &mut ZipWriter<File>,
    archive_name: &str,
    content: &[u8],
    options: SimpleFileOptions,
) -> Result<(), Error> {
    start_entry(writer, archive_name, options)?;
    let written = writer.write_all(content);
    return match written {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(format!(
            "failed to write {archive_name}: {error}"
        ))),
    };
}

/// Write the collected entries into the archive.
fn write_entries(
    writer: &mut ZipWriter<File>,
    entries: &[ArchiveEntry],
    options: SimpleFileOptions,
) -> Result<(), Error> {
    for entry in entries {
        add_source_file(writer, entry, options)?;
    }
    return Ok(());
}

/// Validate one collected file and add it to the archive.
///
/// The file is inspected with [`fs::symlink_metadata`] immediately before
/// it is opened, and the opened handle is checked again through
/// [`File::metadata`]; the content is then streamed from that same handle
/// in fixed-size chunks. A path swapped for a symlink or another file
/// after the collection step is rejected instead of contributing content
/// from elsewhere, except within the check-then-open window.
fn add_source_file(
    writer: &mut ZipWriter<File>,
    entry: &ArchiveEntry,
    options: SimpleFileOptions,
) -> Result<(), Error> {
    let entry_path = &entry.source_path;
    validate_file_type(entry_path)?;
    let opened = File::open(entry_path);
    let mut file = match opened {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to read {}: {error}",
                entry_path.display()
            )));
        }
    };
    let metadata = match file.metadata() {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                entry_path.display()
            )));
        }
    };
    if !metadata.file_type().is_file() {
        return Err(Error::ValidationError(format!(
            "Symlinks or non-regular files are not allowed: {}",
            entry_path.display()
        )));
    }
    start_entry(writer, &entry.archive_name, options)?;
    return copy_stream(writer, &mut file, &entry.archive_name, entry_path);
}

/// Copy `reader` into the current archive entry in fixed-size chunks.
///
/// The content is never held in memory as a whole, so a large source
/// file cannot exhaust memory while it is packed.
fn copy_stream(
    writer: &mut ZipWriter<File>,
    reader: &mut File,
    archive_name: &str,
    source_path: &Path,
) -> Result<(), Error> {
    let mut buffer = [0u8; COPY_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer);
        let count = match read {
            Ok(0) => return Ok(()),
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read {}: {error}",
                    source_path.display()
                )));
            }
        };
        let chunk = match buffer.get(..count) {
            Some(value) => value,
            None => {
                return Err(Error::Runtime(format!(
                    "failed to copy {}: chunk exceeds the buffer",
                    source_path.display()
                )));
            }
        };
        let written = writer.write_all(chunk);
        match written {
            Ok(()) => {}
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to write {archive_name}: {error}"
                )));
            }
        }
    }
}

/// Validate and collect every file below `directory` under `prefix`.
fn collect_entries(
    directory: &Path,
    prefix: &str,
    entries: &mut Vec<ArchiveEntry>,
) -> Result<(), Error> {
    let read = match fs::read_dir(directory) {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to read directory {}: {error}",
                directory.display()
            )));
        }
    };
    let mut read = read;
    loop {
        let next = read.next();
        let entry = match next {
            None => break,
            Some(Ok(value)) => value,
            Some(Err(error)) => {
                return Err(Error::Runtime(format!(
                    "failed to read an entry of {}: {error}",
                    directory.display()
                )));
            }
        };
        collect_entry(&entry, prefix, entries)?;
    }
    return Ok(());
}

/// Validate one directory entry and queue it, recursing into directories.
fn collect_entry(
    entry: &fs::DirEntry,
    prefix: &str,
    entries: &mut Vec<ArchiveEntry>,
) -> Result<(), Error> {
    let entry_path = entry.path();
    validate_file_type(&entry_path)?;
    let file_name = match entry.file_name().into_string() {
        Ok(value) => value,
        Err(_) => {
            return Err(Error::Runtime(format!(
                "file name is not valid Unicode: {}",
                entry_path.display()
            )));
        }
    };
    let archive_name = format!("{prefix}/{file_name}");
    validate_no_node_modules(Path::new(&archive_name))?;
    validate_archive_entry(&archive_name)?;
    let file_type = match entry.file_type() {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                entry_path.display()
            )));
        }
    };
    if file_type.is_dir() {
        return collect_entries(&entry_path, &archive_name, entries);
    }
    if file_type.is_file() {
        entries.push(ArchiveEntry {
            archive_name,
            source_path: entry_path,
        });
        return Ok(());
    }
    return Err(Error::ValidationError(format!(
        "Symlinks or non-regular files are not allowed: {}",
        entry_path.display()
    )));
}
