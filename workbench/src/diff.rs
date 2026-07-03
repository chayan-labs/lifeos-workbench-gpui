//! Line-level diff for the agent review layer (issue #16): agent-proposed
//! file content is split into hunks the user accepts or rejects
//! individually; `apply` rebuilds the file from the accepted subset.

/// One contiguous change: `old` lines (at `old_start`, 0-based) replaced by
/// `new` lines. Pure insert has empty `old`; pure delete has empty `new`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: usize,
    pub old: Vec<String>,
    pub new: Vec<String>,
}

fn lcs_table(a: &[&str], b: &[&str]) -> Vec<Vec<usize>> {
    let mut t = vec![vec![0; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            t[i][j] = if a[i] == b[j] {
                t[i + 1][j + 1] + 1
            } else {
                t[i + 1][j].max(t[i][j + 1])
            };
        }
    }
    t
}

/// Diff `old` -> `new` text into hunks.
pub fn diff(old: &str, new: &str) -> Vec<Hunk> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let t = lcs_table(&a, &b);
    let mut hunks: Vec<Hunk> = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    let mut open: Option<Hunk> = None;
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            if let Some(h) = open.take() {
                hunks.push(h);
            }
            i += 1;
            j += 1;
            continue;
        }
        let h = open.get_or_insert_with(|| Hunk {
            old_start: i,
            old: Vec::new(),
            new: Vec::new(),
        });
        // Follow the LCS table: prefer whichever direction keeps the LCS.
        if j >= b.len() || (i < a.len() && t[i + 1][j] >= t[i][j + 1]) {
            h.old.push(a[i].to_string());
            i += 1;
        } else {
            h.new.push(b[j].to_string());
            j += 1;
        }
    }
    if let Some(h) = open {
        hunks.push(h);
    }
    hunks
}

/// Rebuild the file text from `old`, applying only the `accepted` hunks
/// (indices into `hunks`). Preserves a trailing newline if `old` or any
/// accepted replacement content implies one.
pub fn apply(old: &str, hunks: &[Hunk], accepted: &[usize]) -> String {
    let a: Vec<&str> = old.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    for (idx, h) in hunks.iter().enumerate() {
        while i < h.old_start {
            out.push(a[i].to_string());
            i += 1;
        }
        if accepted.contains(&idx) {
            out.extend(h.new.iter().cloned());
            i += h.old.len();
        }
        // Rejected hunk: keep the old lines (copied by the loop above /
        // below since i is not advanced past them here).
    }
    while i < a.len() {
        out.push(a[i].to_string());
        i += 1;
    }
    let mut text = out.join("\n");
    if !text.is_empty() && (old.ends_with('\n') || old.is_empty()) {
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_finds_replacements_inserts_and_deletes() {
        let old = "a\nb\nc\n";
        let new = "a\nB\nc\nd\n";
        let hunks = diff(old, new);
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].old, vec!["b"]);
        assert_eq!(hunks[0].new, vec!["B"]);
        assert_eq!(hunks[1].old, Vec::<String>::new());
        assert_eq!(hunks[1].new, vec!["d"]);
    }

    #[test]
    fn apply_all_hunks_reproduces_the_new_text() {
        let old = "a\nb\nc\n";
        let new = "x\na\nc\nz\n";
        let hunks = diff(old, new);
        let all: Vec<usize> = (0..hunks.len()).collect();
        assert_eq!(apply(old, &hunks, &all), new);
    }

    #[test]
    fn apply_accepts_hunks_selectively() {
        let old = "a\nb\nc\n";
        let new = "a\nB\nc\nd\n";
        let hunks = diff(old, new);
        // Accept only the replacement, reject the trailing insert.
        assert_eq!(apply(old, &hunks, &[0]), "a\nB\nc\n");
        // Accept only the insert, keep b as-is.
        assert_eq!(apply(old, &hunks, &[1]), "a\nb\nc\nd\n");
        // Reject everything: unchanged.
        assert_eq!(apply(old, &hunks, &[]), old);
    }

    #[test]
    fn diff_of_identical_texts_is_empty() {
        assert!(diff("same\n", "same\n").is_empty());
    }
}
