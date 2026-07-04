//! File navigation (issue #9): an expandable directory tree and a fuzzy
//! file picker over the same walk. Both are cloneable value-state modals in
//! the shell; selecting a file opens it in the focused pane's editor.

use crate::palette::fuzzy_score;
use crossterm::event::KeyCode;
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

#[derive(Clone, Debug)]
pub struct FileTree {
    pub root: PathBuf,
    pub selected: usize,
    expanded: BTreeSet<PathBuf>,
}

/// What a key press did to the tree.
pub enum TreeAction {
    None,
    Close,
    OpenFile(PathBuf),
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

    /// Apply a key, returning the next state and what happened.
    pub fn on_key(&self, code: KeyCode) -> (FileTree, TreeAction) {
        let mut next = self.clone();
        let rows = self.rows();
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return (next, TreeAction::Close),
            KeyCode::Up | KeyCode::Char('k') => next.selected = next.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                if !rows.is_empty() {
                    next.selected = (next.selected + 1).min(rows.len() - 1);
                }
            }
            KeyCode::Enter | KeyCode::Char('l') => {
                if let Some(row) = rows.get(self.selected) {
                    if row.is_dir {
                        if !next.expanded.remove(&row.path) {
                            next.expanded.insert(row.path.clone());
                        }
                    } else {
                        return (next, TreeAction::OpenFile(row.path.clone()));
                    }
                }
            }
            KeyCode::Char('h') => {
                if let Some(row) = rows.get(self.selected) {
                    next.expanded.remove(&row.path);
                }
            }
            _ => {}
        }
        (next, TreeAction::None)
    }
}

/// Recursively list files under a root (bounded, skip-list applied).
pub fn walk_files(root: &Path, max: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for (path, is_dir) in children(&dir) {
            if files.len() >= max {
                return files;
            }
            if is_dir {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Fuzzy file picker modal state, matching against root-relative paths.
#[derive(Clone, Debug)]
pub struct PickerState {
    pub root: PathBuf,
    pub query: String,
    pub selected: usize,
    files: Vec<PathBuf>,
}

pub enum PickerAction {
    None,
    Close,
    OpenFile(PathBuf),
}

impl PickerState {
    pub fn open(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            query: String::new(),
            selected: 0,
            files: walk_files(root, 5000),
        }
    }

    fn relative(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .display()
            .to_string()
    }

    /// Ranked matches as (relative label, absolute path).
    pub fn matches(&self) -> Vec<(String, PathBuf)> {
        let mut scored: Vec<(i32, String, PathBuf)> = self
            .files
            .iter()
            .filter_map(|p| {
                let rel = self.relative(p);
                fuzzy_score(&self.query, &rel).map(|s| (s, rel, p.clone()))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, rel, p)| (rel, p)).collect()
    }

    pub fn on_key(&self, code: KeyCode) -> (PickerState, PickerAction) {
        let mut next = self.clone();
        match code {
            KeyCode::Esc => return (next, PickerAction::Close),
            KeyCode::Enter => {
                let picked = next.matches().get(next.selected).map(|(_, p)| p.clone());
                return match picked {
                    Some(p) => (next, PickerAction::OpenFile(p)),
                    None => (next, PickerAction::Close),
                };
            }
            KeyCode::Up => next.selected = next.selected.saturating_sub(1),
            KeyCode::Down => {
                let n = next.matches().len();
                if n > 0 {
                    next.selected = (next.selected + 1).min(n - 1);
                }
            }
            KeyCode::Backspace => {
                next.query.pop();
                next.selected = 0;
            }
            KeyCode::Char(c) => {
                next.query.push(c);
                next.selected = 0;
            }
            _ => {}
        }
        (next, PickerAction::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        // Unique per call - tests run in parallel in one process.
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("wb_tree_{}_{n}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("README.md"), "hi").unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(root.join(".git/config"), "x").unwrap();
        root
    }

    #[test]
    fn tree_lists_expands_and_opens_files() {
        let root = fixture();
        let tree = FileTree::open(&root);
        let rows = tree.rows();
        // .git skipped; dirs first.
        assert_eq!(rows[0].path, root.join("src"));
        assert!(rows.iter().all(|r| !r.path.ends_with(".git")));

        // Enter on the dir expands it; then j + Enter opens the file inside.
        let (tree, _) = tree.on_key(KeyCode::Enter);
        assert!(tree.rows().iter().any(|r| r.path.ends_with("main.rs")));
        let (tree, _) = tree.on_key(KeyCode::Char('j'));
        let (_, action) = tree.on_key(KeyCode::Enter);
        let TreeAction::OpenFile(path) = action else {
            panic!("expected open");
        };
        assert!(path.ends_with("src/main.rs"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn picker_fuzzy_matches_relative_paths() {
        let root = fixture();
        let picker = PickerState::open(&root);
        let typed = "mainrs"
            .chars()
            .fold(picker, |p, c| p.on_key(KeyCode::Char(c)).0);
        let (_, action) = typed.on_key(KeyCode::Enter);
        let PickerAction::OpenFile(path) = action else {
            panic!("expected open");
        };
        assert!(path.ends_with("src/main.rs"));
        std::fs::remove_dir_all(root).ok();
    }
}
