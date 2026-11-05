//! Iterative directory traversal shared by the commands.
//!
//! A walk keeps the directories it still has to read on the heap, so the
//! nesting depth of a tree cannot exhaust the stack, and it never follows
//! a symlinked directory.

use std::fs;
use std::path::{Path, PathBuf};

use crate::common::Error;
use crate::common::validation::escape_path;

/// One entry reported by [`walk_tree`].
#[non_exhaustive]
pub struct WalkEntry {
    /// Path of the entry on disk.
    pub path: PathBuf,
    /// Path of the entry relative to the walk root.
    pub relative: PathBuf,
    /// File name of the entry.
    pub name: String,
    /// File type reported by the directory entry; a symlink is reported
    /// as a link instead of being followed.
    pub file_type: fs::FileType,
}

/// A directory the walk still has to read.
struct PendingDirectory {
    /// Path of the directory on disk.
    path: PathBuf,
    /// Path of the directory relative to the walk root.
    relative: PathBuf,
}

/// Visit every entry below `root`.
///
/// `visit` returns `true` to descend into a directory; any other entry is
/// reported and skipped.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when a directory or an entry cannot be read,
/// and propagates any error returned by `visit`.
pub fn walk_tree<F>(root: &Path, visit: &mut F) -> Result<(), Error>
where
    F: FnMut(&WalkEntry) -> Result<bool, Error>,
{
    let mut pending: Vec<PendingDirectory> = Vec::new();
    pending.push(PendingDirectory {
        path: root.to_path_buf(),
        relative: PathBuf::new(),
    });
    loop {
        let next = pending.pop();
        let current = match next {
            None => return Ok(()),
            Some(value) => value,
        };
        read_directory(&current, &mut pending, visit)?;
    }
}

/// Read one directory, reporting each entry to `visit`.
fn read_directory<F>(
    current: &PendingDirectory,
    pending: &mut Vec<PendingDirectory>,
    visit: &mut F,
) -> Result<(), Error>
where
    F: FnMut(&WalkEntry) -> Result<bool, Error>,
{
    let read = match fs::read_dir(&current.path) {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to read directory {}: {error}",
                escape_path(&current.path)
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
                    escape_path(&current.path)
                )));
            }
        };
        visit_entry(&entry, current, pending, visit)?;
    }
    return Ok(());
}

/// Report one entry to `visit` and queue its directory when asked to.
fn visit_entry<F>(
    entry: &fs::DirEntry,
    current: &PendingDirectory,
    pending: &mut Vec<PendingDirectory>,
    visit: &mut F,
) -> Result<(), Error>
where
    F: FnMut(&WalkEntry) -> Result<bool, Error>,
{
    let path = entry.path();
    let file_type = match entry.file_type() {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::Runtime(format!(
                "failed to inspect {}: {error}",
                escape_path(&path)
            )));
        }
    };
    let name = match entry.file_name().into_string() {
        Ok(value) => value,
        Err(_) => {
            return Err(Error::Runtime(format!(
                "file name is not valid Unicode: {}",
                escape_path(&path)
            )));
        }
    };
    let relative = current.relative.join(&name);
    let visited = WalkEntry {
        path: path.clone(),
        relative: relative.clone(),
        name,
        file_type,
    };
    let descend = visit(&visited)?;
    if !descend || !file_type.is_dir() {
        return Ok(());
    }
    pending.push(PendingDirectory {
        path,
        relative,
    });
    return Ok(());
}
