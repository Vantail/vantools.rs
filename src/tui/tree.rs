//! ASCII tree rendering for archive listings.
//!
//! The renderer turns a flat list of `/`-separated entry names into the
//! indented tree the `verify` report prints. It knows nothing about the
//! archive format: the caller supplies the names, and every rendered
//! segment is escaped so an untrusted name cannot emit terminal escape
//! sequences.

use std::cmp::Ordering;
use std::path::Path;

use crate::common::validation::escape_path;

/// One node of the rendered tree.
struct TreeNode {
    /// Name of the path segment this node represents.
    name: String,
    /// Children of this node, in sorted order.
    children: Vec<TreeNode>,
}

/// One directory that is open while the tree is built.
struct OpenDirectory {
    /// Name of the path segment this directory represents.
    name: String,
    /// Position of this directory in its parent's children.
    index: usize,
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

/// Render entry names as the lines of an ASCII tree.
///
/// The names are sorted in place with [`compare_paths`], so one ordered
/// pass can build the tree and the caller never holds a second copy of
/// the listing. Every rendered segment is escaped, because an entry
/// name cannot be trusted not to carry control characters.
///
/// # Examples
///
/// Two entries below the same directory are rendered as one directory
/// node with two children, and the directory is marked with a trailing
/// `/`.
pub fn render_tree_lines(names: &mut [String]) -> Vec<String> {
    names.sort_by(compare_paths);
    let tree = build_tree(names);
    return render_tree(&tree);
}

/// Compare two entry names so a directory stays next to its contents.
///
/// A plain byte order separates an entry such as `dist` from the
/// entries below it, `dist/app.ts`, whenever a sibling such as
/// `dist-extra/other.ts` sorts between them, because `/` is `0x2F` and
/// so orders after most punctuation. Comparing `/` below every other
/// byte keeps each directory and everything below it contiguous, which
/// is what [`build_tree`] relies on to reuse an open directory instead
/// of creating a second node with the same name.
fn compare_paths(left: &String, right: &String) -> Ordering {
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    let limit = left_bytes.len().min(right_bytes.len());
    let mut position = 0;
    while position < limit {
        let left_byte = match left_bytes.get(position) {
            Some(value) => *value,
            None => break,
        };
        let right_byte = match right_bytes.get(position) {
            Some(value) => *value,
            None => break,
        };
        let left_key = path_order_key(left_byte);
        let right_key = path_order_key(right_byte);
        if left_key != right_key {
            return left_key.cmp(&right_key);
        }
        position += 1;
    }
    return left_bytes.len().cmp(&right_bytes.len());
}

/// Return the sort key of one byte of an entry name.
///
/// `/` maps below every other byte, so it sorts before any character
/// that could otherwise separate a directory from its contents, and
/// the remaining bytes keep their usual order.
fn path_order_key(byte: u8) -> u16 {
    if byte == b'/' {
        return 0;
    }
    return u16::from(byte) + 1;
}

/// Build the tree of path segments from the ordered entry names.
///
/// [`compare_paths`] keeps every entry contiguous with the entries
/// that share its directory, so one pass can carry the chain of open
/// directories and create only the segments an entry adds. No sibling
/// list is scanned, so a directory holding many entries costs work
/// proportional to the number of path segments instead of a scan per
/// entry.
fn build_tree(names: &[String]) -> TreeNode {
    let mut root = TreeNode {
        name: String::new(),
        children: Vec::new(),
    };
    let mut open: Vec<OpenDirectory> = Vec::new();
    for name in names {
        insert_sorted_path(&mut root, &mut open, name);
    }
    return root;
}

/// Insert one entry whose name belongs after the previous entry.
///
/// `open` holds the directory created for every segment of the
/// previously inserted entry. The segments this entry shares with it
/// are reused, and only the remaining segments are created.
fn insert_sorted_path(
    root: &mut TreeNode,
    open: &mut Vec<OpenDirectory>,
    path: &str,
) {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        segments.push(segment);
    }
    let shared = shared_segments(open, &segments);
    open.truncate(shared);
    let mut position = shared;
    while position < segments.len() {
        let segment = match segments.get(position) {
            Some(value) => value,
            None => return,
        };
        let parent = match node_at_mut(root, open) {
            Some(value) => value,
            None => return,
        };
        let name = String::from(*segment);
        parent.children.push(TreeNode {
            name: name.clone(),
            children: Vec::new(),
        });
        let index = parent.children.len() - 1;
        open.push(OpenDirectory {
            name,
            index,
        });
        position += 1;
    }
}

/// Return the directory reached by following `open` from `root`.
fn node_at_mut<'a>(
    root: &'a mut TreeNode,
    open: &[OpenDirectory],
) -> Option<&'a mut TreeNode> {
    let mut current = root;
    for directory in open {
        current = current.children.get_mut(directory.index)?;
    }
    return Some(current);
}

/// Count the leading segments `open` and `segments` share.
fn shared_segments(open: &[OpenDirectory], segments: &[&str]) -> usize {
    let limit = open.len().min(segments.len());
    let mut shared = 0;
    while shared < limit {
        let directory = match open.get(shared) {
            Some(value) => value,
            None => return shared,
        };
        let segment = match segments.get(shared) {
            Some(value) => value,
            None => return shared,
        };
        if directory.name.as_str() != *segment {
            return shared;
        }
        shared += 1;
    }
    return shared;
}

/// Render the tree below `root` as one line per node.
///
/// Pending nodes are kept on the heap, so the nesting depth of an
/// archive cannot exhaust the stack. A node with children is a
/// directory and is rendered with a trailing `/`.
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
        let mut rendered = escape_path(Path::new(&current.node.name));
        if !current.node.children.is_empty() {
            rendered.push('/');
        }
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
