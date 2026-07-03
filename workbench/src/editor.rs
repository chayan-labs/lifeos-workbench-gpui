//! Editor pane: helix-core does the editing (rope buffer, multi-cursor
//! `Selection`, `Transaction` edits, `History` undo) - we only map keys and
//! draw. Per CLAUDE.md this embeds Helix's core, it does not write an editor.

use crate::highlight::{self, Lang, StyledRange};
use helix_core::doc_formatter::TextFormat;
use helix_core::history::{History, State};
use helix_core::movement::{self, Direction, Movement};
use helix_core::text_annotations::TextAnnotations;
use helix_core::{graphemes, Rope, Selection, Transaction};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
}

/// One open file. Mutable resource-state (like `TermPane`), owned by the
/// `PaneStore`; the layout stays immutable value-state.
pub struct EditorPane {
    pub path: PathBuf,
    doc: Rope,
    selection: Selection,
    pub mode: Mode,
    history: History,
    scroll: usize,
    pub dirty: bool,
    lang: Option<Lang>,
    hl: Vec<StyledRange>,
    hl_stale: bool,
    pending: Option<char>,
    /// (line, severity, message) markers pushed in by the LSP layer
    /// (severity: 1 = error, 2 = warning, 3+ = info/hint).
    pub diagnostics: Vec<(usize, u8, String)>,
    /// One-line transient message (hover result, save confirmation).
    pub message: Option<String>,
    /// LSP request the shell should service (K = hover, gd = definition).
    pub lsp_op: Option<LspOp>,
    /// Bumped on every text change; the LSP layer diffs it for didChange.
    pub version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LspOp {
    Hover,
    Definition,
}

impl EditorPane {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        Ok(Self {
            path: path.to_path_buf(),
            doc: Rope::from(text.as_str()),
            selection: Selection::point(0),
            mode: Mode::Normal,
            history: History::default(),
            scroll: 0,
            dirty: false,
            lang: highlight::detect(path),
            hl: Vec::new(),
            hl_stale: true,
            pending: None,
            diagnostics: Vec::new(),
            message: None,
            lsp_op: None,
            version: 1,
        })
    }

    pub fn text(&self) -> String {
        self.doc.to_string()
    }

    pub fn cursor(&self) -> usize {
        self.selection.primary().cursor(self.doc.slice(..))
    }

    pub fn cursor_line_col(&self) -> (usize, usize) {
        let pos = self.cursor();
        let line = self.doc.char_to_line(pos.min(self.doc.len_chars()));
        (line, pos - self.doc.line_to_char(line))
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        std::fs::write(&self.path, self.doc.to_string())?;
        self.dirty = false;
        self.message = Some(format!("wrote {}", self.path.display()));
        Ok(())
    }

    /// Collapse a mapped selection back to a caret. `Transaction::insert`
    /// inserts at `range.head`, so the head must stay exactly at the caret -
    /// widening to a 1-width block (`ensure_invariants`) would shift every
    /// following insert one char right.
    fn collapse(selection: Selection, doc: &Rope) -> Selection {
        Selection::point(selection.primary().head.min(doc.len_chars()))
    }

    /// Apply a transaction, recording it for undo.
    fn apply(&mut self, tx: Transaction) {
        let original = State {
            doc: self.doc.clone(),
            selection: self.selection.clone(),
        };
        if tx.apply(&mut self.doc) {
            self.selection = Self::collapse(self.selection.clone().map(tx.changes()), &self.doc);
            self.history.commit_revision(&tx, &original);
            self.dirty = true;
            self.hl_stale = true;
            self.version += 1;
        }
    }

    fn time_travel(&mut self, undo: bool) {
        let tx = if undo {
            self.history.undo().cloned()
        } else {
            self.history.redo().cloned()
        };
        if let Some(tx) = tx {
            tx.apply(&mut self.doc);
            self.selection = Self::collapse(self.selection.clone().map(tx.changes()), &self.doc);
            self.dirty = true;
            self.hl_stale = true;
            self.version += 1;
        }
    }

    fn move_cursor(&mut self, horizontal: bool, dir: Direction, count: usize) {
        let slice = self.doc.slice(..);
        let range = self.selection.primary();
        let fmt = TextFormat::default();
        let mut annotations = TextAnnotations::default();
        let next = if horizontal {
            movement::move_horizontally(
                slice,
                range,
                dir,
                count,
                Movement::Move,
                &fmt,
                &mut annotations,
            )
        } else {
            movement::move_vertically(
                slice,
                range,
                dir,
                count,
                Movement::Move,
                &fmt,
                &mut annotations,
            )
        };
        self.selection = Selection::single(next.anchor, next.head);
    }

    fn insert_str(&mut self, s: &str) {
        let tx = Transaction::insert(&self.doc, &self.selection, s.into());
        self.apply(tx);
    }

    fn delete_at_cursor(&mut self) {
        let pos = self.cursor();
        if pos >= self.doc.len_chars() {
            return;
        }
        let end = graphemes::next_grapheme_boundary(self.doc.slice(..), pos);
        let tx = Transaction::delete(&self.doc, std::iter::once((pos, end)));
        self.apply(tx);
    }

    fn backspace(&mut self) {
        let pos = self.cursor();
        if pos == 0 {
            return;
        }
        let start = graphemes::prev_grapheme_boundary(self.doc.slice(..), pos);
        let tx = Transaction::delete(&self.doc, std::iter::once((start, pos)));
        self.apply(tx);
    }

    /// Feed one key. Returns true when the key was consumed.
    pub fn on_key(&mut self, code: crossterm::event::KeyCode, ctrl: bool) -> bool {
        use crossterm::event::KeyCode as K;
        self.message = None;
        if ctrl && code == K::Char('s') {
            let _ = self.save();
            return true;
        }
        match self.mode {
            Mode::Insert => self.on_key_insert(code),
            Mode::Normal => self.on_key_normal(code, ctrl),
        }
    }

    fn on_key_insert(&mut self, code: crossterm::event::KeyCode) -> bool {
        use crossterm::event::KeyCode as K;
        match code {
            K::Esc => self.mode = Mode::Normal,
            K::Enter => self.insert_str("\n"),
            K::Tab => self.insert_str("    "),
            K::Backspace => self.backspace(),
            K::Char(c) => self.insert_str(&c.to_string()),
            K::Left => self.move_cursor(true, Direction::Backward, 1),
            K::Right => self.move_cursor(true, Direction::Forward, 1),
            K::Up => self.move_cursor(false, Direction::Backward, 1),
            K::Down => self.move_cursor(false, Direction::Forward, 1),
            _ => return false,
        }
        true
    }

    fn on_key_normal(&mut self, code: crossterm::event::KeyCode, ctrl: bool) -> bool {
        use crossterm::event::KeyCode as K;
        let slice_range = |pane: &Self| pane.selection.primary();
        if let Some('g') = self.pending.take() {
            match code {
                K::Char('g') => {
                    self.selection = Selection::point(0);
                    return true;
                }
                K::Char('d') => {
                    self.lsp_op = Some(LspOp::Definition);
                    return true;
                }
                _ => {}
            }
        }
        match code {
            K::Char('h') | K::Left => self.move_cursor(true, Direction::Backward, 1),
            K::Char('l') | K::Right => self.move_cursor(true, Direction::Forward, 1),
            K::Char('k') | K::Up => self.move_cursor(false, Direction::Backward, 1),
            K::Char('j') | K::Down => self.move_cursor(false, Direction::Forward, 1),
            K::Char('w') => {
                let r = movement::move_next_word_start(self.doc.slice(..), slice_range(self), 1);
                self.selection = Selection::point(r.cursor(self.doc.slice(..)));
            }
            K::Char('b') => {
                let r = movement::move_prev_word_start(self.doc.slice(..), slice_range(self), 1);
                self.selection = Selection::point(r.cursor(self.doc.slice(..)));
            }
            K::Char('e') => {
                let r = movement::move_next_word_end(self.doc.slice(..), slice_range(self), 1);
                self.selection = Selection::point(r.cursor(self.doc.slice(..)));
            }
            K::Char('0') => {
                let (line, _) = self.cursor_line_col();
                self.selection = Selection::point(self.doc.line_to_char(line));
            }
            K::Char('$') => {
                let (line, _) = self.cursor_line_col();
                let end = self.doc.line_to_char(line) + self.doc.line(line).len_chars();
                self.selection = Selection::point(end.saturating_sub(1));
            }
            K::Char('g') => self.pending = Some('g'),
            K::Char('G') => self.selection = Selection::point(self.doc.len_chars()),
            K::Char('i') => self.mode = Mode::Insert,
            K::Char('a') => {
                self.move_cursor(true, Direction::Forward, 1);
                self.mode = Mode::Insert;
            }
            K::Char('o') => {
                // Insert a newline at the end of the line's content; the
                // selection maps to just after it - a fresh line below.
                let (line, _) = self.cursor_line_col();
                let l = self.doc.line(line);
                let trailing = usize::from(l.len_chars() > 0 && l.char(l.len_chars() - 1) == '\n');
                let eol = self.doc.line_to_char(line) + l.len_chars() - trailing;
                self.selection = Selection::point(eol);
                self.insert_str("\n");
                self.mode = Mode::Insert;
            }
            K::Char('x') => self.delete_at_cursor(),
            K::Char('K') => self.lsp_op = Some(LspOp::Hover),
            K::Char('u') => self.time_travel(true),
            K::Char('r') if ctrl => self.time_travel(false),
            K::Char('U') => self.time_travel(false),
            _ => return false,
        }
        true
    }

    /// Columns consumed by the gutter (severity marker + line number + gap),
    /// so mouse hits can be translated into content columns.
    pub fn gutter_cols(&self) -> usize {
        1 + self.doc.len_lines().to_string().len().max(3) + 1
    }

    /// Place the cursor from a mouse hit: `viewport_row` is relative to the
    /// rendered rows (scroll applied), `content_col` is past the gutter.
    pub fn on_click(&mut self, viewport_row: usize, content_col: usize) {
        let total = self.doc.len_lines();
        let line = (self.scroll + viewport_row).min(total.saturating_sub(1));
        let l = self.doc.line(line);
        let has_newline = l.len_chars() > 0 && l.char(l.len_chars() - 1) == '\n';
        let max_col = l.len_chars().saturating_sub(usize::from(has_newline));
        let col = content_col.min(max_col);
        self.selection = Selection::point(self.doc.line_to_char(line) + col);
    }

    /// Wheel scroll: move the cursor vertically so the auto-follow scroll in
    /// `render_lines` brings the view along without fighting the wheel.
    pub fn on_scroll(&mut self, down: bool) {
        let dir = if down {
            Direction::Forward
        } else {
            Direction::Backward
        };
        self.move_cursor(false, dir, 3);
    }

    fn refresh_highlight(&mut self) {
        if !self.hl_stale {
            return;
        }
        self.hl = match self.lang {
            Some(lang) => highlight::highlight(lang, &self.doc.to_string()),
            None => Vec::new(),
        };
        self.hl_stale = false;
    }

    /// Render `height` rows, scrolling to keep the cursor visible.
    pub fn render_lines(&mut self, height: usize) -> Vec<Line<'static>> {
        self.refresh_highlight();
        let (cursor_line, cursor_col) = self.cursor_line_col();
        if cursor_line < self.scroll {
            self.scroll = cursor_line;
        } else if height > 0 && cursor_line >= self.scroll + height {
            self.scroll = cursor_line + 1 - height;
        }
        let total = self.doc.len_lines();
        let gutter_width = total.to_string().len().max(3);
        let diag_lines: Vec<(usize, u8)> =
            self.diagnostics.iter().map(|(l, s, _)| (*l, *s)).collect();
        (self.scroll..total.min(self.scroll + height))
            .map(|row| {
                self.render_row(
                    row,
                    gutter_width,
                    &diag_lines,
                    row == cursor_line,
                    cursor_col,
                )
            })
            .collect()
    }

    /// Scroll offset and total line count, for the marked scrollbar strip.
    pub fn scroll_info(&self) -> (usize, usize) {
        (self.scroll, self.doc.len_lines())
    }

    fn render_row(
        &self,
        row: usize,
        gutter_width: usize,
        diag_lines: &[(usize, u8)],
        is_cursor_row: bool,
        cursor_col: usize,
    ) -> Line<'static> {
        let severity = diag_lines
            .iter()
            .filter(|(l, _)| *l == row)
            .map(|(_, s)| *s)
            .min();
        let (marker, marker_style) = match severity {
            Some(s) => (
                "●",
                ratatui::style::Style::default().fg(crate::decorations::severity_color(s)),
            ),
            None => (
                " ",
                ratatui::style::Style::default().fg(ratatui::style::Color::Indexed(101)),
            ),
        };
        let mut spans = vec![
            Span::styled(marker.to_string(), marker_style),
            Span::styled(
                format!("{:>gutter_width$} ", row + 1),
                ratatui::style::Style::default().fg(ratatui::style::Color::Indexed(101)),
            ),
        ];
        let line_start_char = self.doc.line_to_char(row);
        let line = self.doc.line(row);
        for (i, ch) in line.chars().enumerate() {
            if ch == '\n' && !(is_cursor_row && i == cursor_col) {
                continue;
            }
            let byte = self.doc.char_to_byte(line_start_char + i);
            let mut style = highlight::style_at(&self.hl, byte).unwrap_or_default();
            if let Some(s) = severity {
                style = style.patch(crate::decorations::squiggle(s));
            }
            if is_cursor_row && i == cursor_col {
                style = style.add_modifier(Modifier::REVERSED);
            }
            let shown = if ch == '\n' { ' ' } else { ch };
            spans.push(Span::styled(shown.to_string(), style));
        }
        if is_cursor_row && cursor_col >= line.len_chars() {
            spans.push(Span::styled(
                " ".to_string(),
                ratatui::style::Style::default().add_modifier(Modifier::REVERSED),
            ));
        }
        Line::from(spans)
    }

    /// Statusline fragment: `INSERT · src/main.rs [+]`.
    pub fn status(&self) -> String {
        let mode = match self.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
        };
        let dirty = if self.dirty { " [+]" } else { "" };
        match &self.message {
            Some(m) => format!("{mode} · {}{dirty} · {m}", self.path.display()),
            None => format!("{mode} · {}{dirty}", self.path.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode as K;

    fn pane_with(content: &str) -> (EditorPane, PathBuf) {
        // Unique per call - tests run in parallel in one process.
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("wb_editor_{}_{n}.rs", std::process::id()));
        std::fs::write(&path, content).unwrap();
        (EditorPane::open(&path).unwrap(), path)
    }

    #[test]
    fn opens_edits_and_saves_a_file_with_modal_editing() {
        let (mut ed, path) = pane_with("fn main() {}\n");
        assert_eq!(ed.mode, Mode::Normal);
        // 'i' -> insert "// " at start, Esc back to normal.
        ed.on_key(K::Char('i'), false);
        assert_eq!(ed.mode, Mode::Insert);
        for c in "// ".chars() {
            ed.on_key(K::Char(c), false);
        }
        ed.on_key(K::Esc, false);
        assert_eq!(ed.mode, Mode::Normal);
        assert!(ed.text().starts_with("// fn main"));
        assert!(ed.dirty);
        ed.save().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), ed.text());
        assert!(!ed.dirty);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn movement_delete_and_undo_round_trip() {
        let (mut ed, path) = pane_with("abc\ndef\n");
        ed.on_key(K::Char('l'), false);
        assert_eq!(ed.cursor(), 1);
        ed.on_key(K::Char('j'), false);
        assert_eq!(ed.cursor_line_col().0, 1);
        ed.on_key(K::Char('g'), false);
        ed.on_key(K::Char('g'), false);
        assert_eq!(ed.cursor(), 0);
        ed.on_key(K::Char('x'), false);
        assert_eq!(ed.text(), "bc\ndef\n");
        ed.on_key(K::Char('u'), false);
        assert_eq!(ed.text(), "abc\ndef\n", "undo restores");
        ed.on_key(K::Char('U'), false);
        assert_eq!(ed.text(), "bc\ndef\n", "redo re-applies");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn renders_highlighted_lines_with_cursor_and_gutter() {
        let (mut ed, path) = pane_with("fn main() {\n    let x = 1;\n}\n");
        let lines = ed.render_lines(10);
        assert!(lines.len() >= 3);
        let first: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(first.contains("1 fn main"), "gutter + text: {first}");
        // Gutter is two spans: severity marker (blank here) + line number.
        assert_eq!(lines[0].spans[0].content.as_ref(), " ");
        assert!(lines[0].spans[1].content.contains('1'));
        // The 'f' of fn is keyword-styled and cursor-reversed.
        let fn_span = &lines[0].spans[2];
        assert!(fn_span.style.add_modifier.contains(Modifier::REVERSED));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn diagnostic_rows_render_severity_marker_and_underline() {
        let (mut ed, path) = pane_with("let x = 1;\nok\n");
        ed.diagnostics = vec![(0, 1, "boom".to_string())];
        let lines = ed.render_lines(10);
        // Row 0: error marker in severity color, text underlined.
        assert_eq!(lines[0].spans[0].content.as_ref(), "●");
        assert_eq!(
            lines[0].spans[0].style.fg,
            Some(crate::decorations::severity_color(1))
        );
        let text_span = &lines[0].spans[2];
        assert!(text_span.style.add_modifier.contains(Modifier::UNDERLINED));
        // Row 1 is clean: blank marker, no underline.
        assert_eq!(lines[1].spans[0].content.as_ref(), " ");
        assert!(!lines[1].spans[2]
            .style
            .add_modifier
            .contains(Modifier::UNDERLINED));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn click_places_the_cursor_and_clamps_to_line_ends() {
        let (mut ed, path) = pane_with("abc\nde\n");
        ed.on_click(1, 1);
        assert_eq!(ed.cursor_line_col(), (1, 1));
        // Past end of line clamps to just after the last char.
        ed.on_click(0, 99);
        assert_eq!(ed.cursor_line_col(), (0, 3));
        // Past end of document clamps to the last line.
        ed.on_click(99, 0);
        assert_eq!(ed.cursor_line_col().0, 2);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn wheel_scroll_moves_the_cursor_three_lines() {
        let content: String = (0..20).map(|i| format!("line {i}\n")).collect();
        let (mut ed, path) = pane_with(&content);
        ed.on_scroll(true);
        assert_eq!(ed.cursor_line_col().0, 3);
        ed.on_scroll(false);
        assert_eq!(ed.cursor_line_col().0, 0);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn gutter_width_tracks_line_count() {
        let (ed, path) = pane_with("a\n");
        // marker(1) + max(3 digits) + gap(1)
        assert_eq!(ed.gutter_cols(), 5);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn scroll_follows_the_cursor() {
        let content: String = (0..50).map(|i| format!("line {i}\n")).collect();
        let (mut ed, path) = pane_with(&content);
        ed.on_key(K::Char('G'), false);
        let lines = ed.render_lines(5);
        let last: String = lines
            .last()
            .unwrap()
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(last.contains("51") || last.contains("line 49"), "{last}");
        std::fs::remove_file(path).ok();
    }
}
