//! The sidebar's expandable directory tree.
//!
//! Ported from the legacy `file_tree.rs`, dropping the crossterm `on_key`
//! reducer: gpui drives the tree by mouse (click a row to select, click a
//! directory to expand/collapse), so the state exposes immutable mutators
//! (`toggled`, `selected`) instead of a key handler. The fuzzy file picker is a
//! later addition; this is the always-visible navigation surface.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", ".direnv", "dist"];

fn skip(name: &str) -> bool {
    SKIP_DIRS.contains(&name)
}

fn children(dir: &Path) -> Vec<(PathBuf, bool)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(PathBuf, bool)> = entries
        .flatten()
        .map(|e| {
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (e.path(), is_dir)
        })
        .filter(|(p, _)| !p.file_name().and_then(|n| n.to_str()).is_some_and(skip))
        .collect();
    // Directories first, then names, both alphabetical.
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

/// One visible row of the flattened tree.
#[derive(Clone, Debug)]
pub struct TreeRow {
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

impl TreeRow {
    /// The bare file/directory name for display.
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string()
    }
}

#[derive(Clone, Debug)]
pub struct FileTree {
    pub root: PathBuf,
    pub selected: usize,
    expanded: BTreeSet<PathBuf>,
}

impl FileTree {
    pub fn open(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            selected: 0,
            expanded: BTreeSet::new(),
        }
    }

    /// Flatten the tree to visible rows (expanded dirs recurse).
    pub fn rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        self.walk(&self.root, 0, &mut rows);
        rows
    }

    fn walk(&self, dir: &Path, depth: usize, rows: &mut Vec<TreeRow>) {
        for (path, is_dir) in children(dir) {
            let expanded = is_dir && self.expanded.contains(&path);
            rows.push(TreeRow {
                path: path.clone(),
                is_dir,
                depth,
                expanded,
            });
            if expanded {
                self.walk(&path, depth + 1, rows);
            }
        }
    }

    /// Toggle a directory's expansion (no-op for files); returns a new tree.
    pub fn toggled(&self, path: &Path) -> FileTree {
        let mut next = self.clone();
        if !next.expanded.remove(path) {
            next.expanded.insert(path.to_path_buf());
        }
        next
    }

    /// Select the row at `index` (clamped); returns a new tree.
    pub fn selected(&self, index: usize) -> FileTree {
        let mut next = self.clone();
        let n = self.rows().len();
        next.selected = if n == 0 { 0 } else { index.min(n - 1) };
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        // Unique per call - tests run in parallel in one process.
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("wb_uitree_{}_{n}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("README.md"), "hi").unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(root.join(".git/config"), "x").unwrap();
        root
    }

    #[test]
    fn tree_lists_dirs_first_and_skips_ignored() {
        let root = fixture();
        let tree = FileTree::open(&root);
        let rows = tree.rows();
        assert_eq!(rows[0].path, root.join("src"));
        assert!(rows.iter().all(|r| !r.path.ends_with(".git")));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn toggling_a_dir_expands_and_collapses_it() {
        let root = fixture();
        let tree = FileTree::open(&root);
        let src = root.join("src");
        let tree = tree.toggled(&src);
        assert!(tree.rows().iter().any(|r| r.path.ends_with("main.rs")));
        let tree = tree.toggled(&src);
        assert!(tree.rows().iter().all(|r| !r.path.ends_with("main.rs")));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn selected_clamps_to_visible_rows() {
        let root = fixture();
        let tree = FileTree::open(&root).selected(999);
        assert_eq!(tree.selected, tree.rows().len() - 1);
        std::fs::remove_dir_all(root).ok();
    }
}
