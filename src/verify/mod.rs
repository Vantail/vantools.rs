//! Implementation of the `verify` command.
//!
//! The command opens an existing `.van` archive, validates every entry
//! name with the shared archive rules, rejects symbolic links, and
//! prints the archive contents as a tree. Nothing is ever extracted.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::common::Error;
use crate::common::cli::CliCommand;
use crate::common::command::Command;
use crate::common::output::write_line;
use crate::common::validation::{
    escape_path, escape_text, validate_archive_entry, validate_no_node_modules,
};

mod cli;

pub use cli::run_cli;

/// Mask that selects the file-type bits of a Unix mode.
const FILE_TYPE_MASK: u32 = 0o170000;

/// File-type bits of a symbolic link in a Unix mode.
const SYMLINK_FILE_TYPE: u32 = 0o120000;

/// The `verify` command.
///
/// Opens the archive at `source` and validates its structure without
/// extracting it: every entry name must be a safe relative path, no
/// entry may carry a `node_modules` component, and no entry may be a
/// symbolic link. A valid archive is printed as a tree.
#[non_exhaustive]
pub struct VerifyCommand {
    /// The `.van` archive to verify.
    pub source: PathBuf,
}

/// One node of the archive tree.
struct TreeNode {
    /// Name of the path segment this node represents.
    name: String,
    /// Children of this node, in sorted order.
    children: Vec<TreeNode>,
}

/// A node waiting to be rendered.
struct PendingNode<'a> {
    /// The node to render.
    node: &'a TreeNode,
    /// The prefix shared by every line below this node.
    prefix: String,
    /// Whether this node is the last child of its parent.
    is_last: bool,
}

impl Command for VerifyCommand {
    fn run(&self) -> Result<(), Error> {
        let names = self.collect_entry_names()?;
        return print_tree(&self.source, names);
    }
}

impl VerifyCommand {
    /// Creates a `verify` command for the given archive.
    pub fn new(source: PathBuf) -> Self {
        return Self {
            source,
        };
    }

    /// Open the archive and validate every entry name.
    ///
    /// The entries are validated in archive order and the first invalid
    /// entry ends the run, so an invalid archive is never partly
    /// printed.
    fn collect_entry_names(&self) -> Result<Vec<String>, Error> {
        let file = match File::open(&self.source) {
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to open archive {}: {error}",
                    escape_path(&self.source)
                )));
            }
        };
        let opened = ZipArchive::new(file);
        let mut archive = match opened {
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Runtime(format!(
                    "failed to read archive {}: {error}",
                    escape_path(&self.source)
                )));
            }
        };
        let count = archive.len();
        let mut names: Vec<String> = Vec::new();
        for index in 0..count {
            let entry = archive.by_index(index);
            let entry = match entry {
                Ok(value) => value,
                Err(error) => {
                    return Err(Error::Runtime(format!(
                        "failed to read entry {index} of {}: {error}",
                        escape_path(&self.source)
                    )));
                }
            };
            let name = match entry.name() {
                Ok(value) => value.into_owned(),
                Err(error) => {
                    return Err(Error::Runtime(format!(
                        "failed to read the name of entry {index} of {}: {error}",
                        escape_path(&self.source)
                    )));
                }
            };
            validate_archive_entry(&name)?;
            validate_no_node_modules(Path::new(&name))?;
            reject_symlink(&name, entry.unix_mode())?;
            names.push(name);
        }
        return Ok(names);
    }
}

impl CliCommand for VerifyCommand {
    fn name(&self) -> &'static str {
        return "verify";
    }

    fn usage(&self) -> &'static str {
        return "verify -s <source>";
    }

    fn help(&self) -> &'static str {
        return "Usage: verify -s <source>\n\nOptions:\n  -s, --source <path>  The .van archive to verify\n  -h, --help           Print this help message\n  -v, --version        Print version information";
    }
}

/// Return an error when the entry mode marks a symbolic link.
///
/// The mode comes from the ZIP external attributes and is absent for
/// archives written on platforms without Unix modes, so a missing mode
/// passes. Only the file-type bits are inspected: the permission bits
/// of a regular entry are left alone.
fn reject_symlink(name: &str, mode: Option<u32>) -> Result<(), Error> {
    let mode = match mode {
        Some(value) => value,
        None => return Ok(()),
    };
    if mode & FILE_TYPE_MASK == SYMLINK_FILE_TYPE {
        return Err(Error::ValidationError(format!(
            "Symlinks are not allowed in the archive: {}",
            escape_text(name)
        )));
    }
    return Ok(());
}

/// Print the validated archive as a tree.
///
/// The header names the archive, and every entry is rendered below it
/// with its parent directories. `names` is sorted so that the tree
/// groups the entries of one directory together.
fn print_tree(source: &Path, mut names: Vec<String>) -> Result<(), Error> {
    names.sort();
    let tree = build_tree(&names);
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let header = format!("Archive: {}", archive_label(source));
    write_line(&mut handle, &header, "output")?;
    let lines = render_tree(&tree);
    for line in lines {
        write_line(&mut handle, &line, "output")?;
    }
    return Ok(());
}

/// Render the archive file name used in the tree header.
fn archive_label(source: &Path) -> String {
    let name = match source.file_name() {
        Some(value) => value,
        None => return escape_path(source),
    };
    return escape_path(Path::new(name));
}

/// Build the tree of path segments from the sorted entry names.
fn build_tree(names: &[String]) -> TreeNode {
    let mut root = TreeNode {
        name: String::new(),
        children: Vec::new(),
    };
    for name in names {
        insert_path(&mut root, name);
    }
    return root;
}

/// Insert one entry into `root`, creating the missing directories.
fn insert_path(root: &mut TreeNode, path: &str) {
    let mut current = root;
    for segment in path.split('/') {
        let position = child_position(current, segment);
        let position = match position {
            Some(value) => value,
            None => {
                current.children.push(TreeNode {
                    name: String::from(segment),
                    children: Vec::new(),
                });
                current.children.len() - 1
            }
        };
        let next = current.children.get_mut(position);
        current = match next {
            Some(value) => value,
            None => return,
        };
    }
}

/// Return the position of the child named `name`, when it exists.
fn child_position(node: &TreeNode, name: &str) -> Option<usize> {
    for (position, child) in node.children.iter().enumerate() {
        if child.name == name {
            return Some(position);
        }
    }
    return None;
}

/// Render the tree below `root` as one line per node.
///
/// Pending nodes are kept on the heap, so the nesting depth of an
/// archive cannot exhaust the stack.
fn render_tree(root: &TreeNode) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut pending: Vec<PendingNode> = Vec::new();
    queue_children(root, "", &mut pending);
    loop {
        let next = pending.pop();
        let current = match next {
            None => return lines,
            Some(value) => value,
        };
        let connector = match current.is_last {
            true => "└── ",
            false => "├── ",
        };
        let rendered = escape_path(Path::new(&current.node.name));
        lines.push(format!("{}{connector}{rendered}", current.prefix));
        let child_prefix = format!(
            "{}{}",
            current.prefix,
            match current.is_last {
                true => "    ",
                false => "│   ",
            }
        );
        queue_children(current.node, &child_prefix, &mut pending);
    }
}

/// Queue the children of `node` so the first child is rendered first.
fn queue_children<'a>(
    node: &'a TreeNode,
    prefix: &str,
    pending: &mut Vec<PendingNode<'a>>,
) {
    let count = node.children.len();
    let mut position = count;
    for child in node.children.iter().rev() {
        let is_last = position == count;
        pending.push(PendingNode {
            node: child,
            prefix: String::from(prefix),
            is_last,
        });
        position -= 1;
    }
}
