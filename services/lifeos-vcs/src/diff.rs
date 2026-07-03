//! Per-type semantic diff dispatch (issue #85, docs/VERSIONING.md §3).
//!
//! Only the text strategy is a real, working implementation here - it's the
//! one modality that needs no external pipeline ("a raw byte diff is
//! useless for media", but text/code/markdown/Godot `.tscn`/`.tres` *are*
//! text, so a real line diff is "first-class scene history for free," per
//! the docs). Every other row in §3's table depends on a pipeline this repo
//! hasn't built yet - `lifeos-ingest` transcription/captioning (#88-90) or
//! `mcp-figma` - so `strategy_for` routes them to `DiffStrategy::Unsupported`
//! with the specific blocking issue named, rather than a silent no-op.
//!
//! The Haiku-written plain-English summary (§3) is an orchestration concern
//! that lives in `server/` (the same Agent SDK call `scaffold.js` already
//! makes for structured output, issue #72) - `TextDiffResult::summary()`
//! here is the deterministic line-count summary the AI-written one would be
//! built from, not a replacement for it.

use std::fmt;

use serde::Serialize;
use similar::{ChangeTag, TextDiff};

use crate::blob::read_blob;
use crate::store::ObjectStore;

#[derive(Debug)]
pub enum DiffError {
    Io(std::io::Error),
    NotUtf8Text,
    UnsupportedKind { kind: String, blocked_by: &'static str },
}

impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffError::Io(e) => write!(f, "io error: {e}"),
            DiffError::NotUtf8Text => write!(f, "blob is not valid UTF-8 text"),
            DiffError::UnsupportedKind { kind, blocked_by } => {
                write!(f, "no diff pipeline for entity type \"{kind}\" yet - blocked by {blocked_by}")
            }
        }
    }
}

impl std::error::Error for DiffError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffStrategy {
    Text,
    Unsupported,
}

/// Routes an entity type to its diff strategy, per docs/VERSIONING.md §3's table.
pub fn strategy_for(entity_type: &str) -> DiffStrategy {
    match entity_type {
        "text" | "code" | "markdown" | "godot_tscn" | "godot_tres" => DiffStrategy::Text,
        _ => DiffStrategy::Unsupported,
    }
}

fn blocking_issue_for(entity_type: &str) -> &'static str {
    match entity_type {
        "image" => "lifeos-ingest image captioning (#90) + a perceptual-hash/pixel-diff crate",
        "video" => "lifeos-ingest transcription/segment extraction (#89)",
        "audio" => "lifeos-ingest transcription (#89)",
        "figma" => "mcp-figma node-tree wiring",
        "pdf" | "docx" => "lifeos-ingest text extraction (#90) - the extracted text then reuses the Text strategy",
        _ => "no diff pipeline defined for this entity type yet",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineTag {
    Equal,
    Insert,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffLine {
    pub tag: LineTag,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextDiffResult {
    pub lines: Vec<DiffLine>,
    pub inserted: usize,
    pub deleted: usize,
}

impl TextDiffResult {
    /// Deterministic plain-English summary, e.g. "3 lines added, 1 removed".
    pub fn summary(&self) -> String {
        match (self.inserted, self.deleted) {
            (0, 0) => "no changes".to_string(),
            (i, 0) => format!("{i} line{} added", plural(i)),
            (0, d) => format!("{d} line{} removed", plural(d)),
            (i, d) => format!("{i} line{} added, {d} line{} removed", plural(i), plural(d)),
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Line-level diff via `similar`'s Myers-diff implementation.
pub fn diff_text(old: &str, new: &str) -> TextDiffResult {
    let diff = TextDiff::from_lines(old, new);
    let mut lines = Vec::new();
    let mut inserted = 0;
    let mut deleted = 0;

    for change in diff.iter_all_changes() {
        let tag = match change.tag() {
            ChangeTag::Equal => LineTag::Equal,
            ChangeTag::Insert => {
                inserted += 1;
                LineTag::Insert
            }
            ChangeTag::Delete => {
                deleted += 1;
                LineTag::Delete
            }
        };
        lines.push(DiffLine { tag, text: change.to_string() });
    }

    TextDiffResult { lines, inserted, deleted }
}

/// Dispatches a diff between two blob versions of an entity by its type.
/// `old_ref`/`new_ref` are blob_refs (issue #81) - each is read back via
/// `read_blob` before diffing, never the raw manifest bytes.
pub fn diff_blobs(
    store: &ObjectStore,
    old_ref: &str,
    new_ref: &str,
    entity_type: &str,
) -> Result<TextDiffResult, DiffError> {
    match strategy_for(entity_type) {
        DiffStrategy::Text => {
            let old_bytes = read_blob(store, old_ref).map_err(DiffError::Io)?;
            let new_bytes = read_blob(store, new_ref).map_err(DiffError::Io)?;
            let old_text = String::from_utf8(old_bytes).map_err(|_| DiffError::NotUtf8Text)?;
            let new_text = String::from_utf8(new_bytes).map_err(|_| DiffError::NotUtf8Text)?;
            Ok(diff_text(&old_text, &new_text))
        }
        DiffStrategy::Unsupported => Err(DiffError::UnsupportedKind {
            kind: entity_type.to_string(),
            blocked_by: blocking_issue_for(entity_type),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blob::store_blob;

    #[test]
    fn diff_text_reports_line_level_insertions_and_deletions() {
        let old = "line one\nline two\nline three\n";
        let new = "line one\nline two changed\nline three\nline four\n";

        let result = diff_text(old, new);

        assert_eq!(result.deleted, 1);
        assert_eq!(result.inserted, 2);
        assert!(result.lines.iter().any(|l| l.tag == LineTag::Equal && l.text.contains("line one")));
    }

    #[test]
    fn diff_text_summary_covers_no_change_add_remove_and_mixed() {
        assert_eq!(diff_text("same\n", "same\n").summary(), "no changes");
        assert_eq!(diff_text("a\n", "a\nb\n").summary(), "1 line added");
        assert_eq!(diff_text("a\nb\n", "a\n").summary(), "1 line removed");
        assert_eq!(diff_text("a\n", "b\nc\n").summary(), "2 lines added, 1 line removed");
    }

    #[test]
    fn diff_blobs_computes_a_real_diff_for_text_backed_types() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let old_ref = store_blob(&store, b"fn main() {}\n").unwrap();
        let new_ref = store_blob(&store, b"fn main() {\n    println!(\"hi\");\n}\n").unwrap();

        let result = diff_blobs(&store, &old_ref, &new_ref, "code").unwrap();

        // No shared line between "fn main() {}\n" and "fn main() {\n" -
        // the whole old line is deleted and all 3 new lines are inserted.
        assert_eq!(result.inserted, 3);
        assert_eq!(result.deleted, 1);
        assert_eq!(result.summary(), "3 lines added, 1 line removed");
    }

    #[test]
    fn godot_scene_files_get_a_real_text_diff_for_free() {
        assert_eq!(strategy_for("godot_tscn"), DiffStrategy::Text);
        assert_eq!(strategy_for("godot_tres"), DiffStrategy::Text);
    }

    #[test]
    fn diff_blobs_names_the_blocking_issue_for_unsupported_media_types() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let old_ref = store_blob(&store, b"fake image bytes").unwrap();
        let new_ref = store_blob(&store, b"different fake image bytes").unwrap();

        let result = diff_blobs(&store, &old_ref, &new_ref, "image");

        match result {
            Err(DiffError::UnsupportedKind { kind, blocked_by }) => {
                assert_eq!(kind, "image");
                assert!(blocked_by.contains("#90"));
            }
            other => panic!("expected UnsupportedKind, got {other:?}"),
        }
    }
}
