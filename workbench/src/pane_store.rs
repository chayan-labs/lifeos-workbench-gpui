//! Owns the live pane contents behind the layout tree. The layout is
//! immutable value-state; ptys, editor buffers, and LSP children are real OS
//! resources, so they live here and are reconciled every frame against the
//! layout rects and the shell's desired pane kinds. A pane keeps BOTH its
//! terminal and its editor alive, so flipping terminal<->editor is instant
//! and the shell session (cwd, env, running job) survives the round trip.

use crate::agent_pane::AgentPane;
use crate::api::InProcessApi;
use crate::editor::{EditorPane, LspOp};
use crate::layout::PaneId;
use crate::lifeos_pane::LifeOsPane;
use crate::lsp::{server_for, LspClient};
use crate::search_pane::SearchPane;
use crate::shell::PaneDesire;
use crate::term_pane::TermPane;
use ratatui::layout::Rect;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Default)]
pub struct PaneStore {
    panes: HashMap<PaneId, Entry>,
    /// One client per server binary, shared by every editor pane; `None`
    /// means "tried, not installed" so we never respawn each frame.
    lsp: HashMap<&'static str, Option<Arc<LspClient>>>,
    root: PathBuf,
    /// In-process lifeos-api handle for panes that query Life OS.
    api: Option<InProcessApi>,
}

struct Entry {
    term: Option<TermPane>,
    editor: Option<EditorPane>,
    agent: Option<AgentPane>,
    search: Option<SearchPane>,
    lifeos: Option<LifeOsPane>,
    size: (u16, u16),
    lsp_synced_version: u64,
}

/// A pane's drawable interior: below the header row, left of the reserved
/// right-edge column (scrollbar / pane seam).
fn inner(rect: Rect) -> (u16, u16) {
    (
        rect.width.saturating_sub(1).max(1),
        rect.height.saturating_sub(1).max(1),
    )
}

impl PaneStore {
    pub fn new(root: &Path, api: Option<InProcessApi>) -> Self {
        Self {
            root: root.to_path_buf(),
            api,
            ..Self::default()
        }
    }

    /// Reconcile with the layout: spawn/resize/reap terminals, open editors
    /// for panes whose desire names a file, and service LSP traffic.
    pub fn sync(&mut self, rects: &[(PaneId, Rect)], desires: &HashMap<PaneId, PaneDesire>) {
        let live: Vec<PaneId> = rects.iter().map(|(id, _)| *id).collect();
        // A pane survives while it is on screen OR still desired (panes of
        // background tabs keep their shells/editors across tab switches).
        self.panes
            .retain(|id, _| live.contains(id) || desires.contains_key(id));
        for (id, rect) in rects {
            let (cols, rows) = inner(*rect);
            let entry = self.panes.entry(*id).or_insert_with(|| Entry {
                term: None,
                editor: None,
                agent: None,
                search: None,
                lifeos: None,
                size: (0, 0),
                lsp_synced_version: 0,
            });
            if entry.size != (cols, rows) {
                entry.size = (cols, rows);
                if let Some(term) = entry.term.as_mut() {
                    term.resize(cols, rows);
                }
            }
            match desires.get(id) {
                Some(PaneDesire::Editor(path)) => {
                    let stale = entry.editor.as_ref().is_none_or(|e| e.path != *path);
                    if stale {
                        entry.editor = EditorPane::open(path).ok();
                        entry.lsp_synced_version = 0;
                    }
                }
                Some(PaneDesire::Agent) => {
                    if entry.agent.is_none() {
                        entry.agent = Some(AgentPane::spawn(&self.root));
                    }
                }
                Some(PaneDesire::Search) => {
                    if entry.search.is_none() {
                        entry.search = self
                            .api
                            .as_ref()
                            .map(|api| SearchPane::new(api.clone(), None));
                    }
                }
                Some(PaneDesire::LifeOs) => {
                    if entry.lifeos.is_none() {
                        entry.lifeos = self
                            .api
                            .as_ref()
                            .map(|api| LifeOsPane::new(api.clone(), None));
                    }
                }
                // Welcome panes hold no OS resources until a desire lands.
                Some(PaneDesire::Welcome) => {}
                _ => {
                    if entry.term.is_none() {
                        entry.term = TermPane::spawn(None, cols, rows).ok();
                    }
                }
            }
        }
        self.sync_lsp();
    }

    fn client_for(
        lsp: &mut HashMap<&'static str, Option<Arc<LspClient>>>,
        root: &Path,
        path: &Path,
    ) -> Option<(Arc<LspClient>, &'static str)> {
        let (program, language_id) = server_for(path)?;
        let slot = lsp
            .entry(program)
            .or_insert_with(|| LspClient::spawn(program, root).map(Arc::new));
        slot.as_ref().map(|c| (c.clone(), language_id))
    }

    /// Push editor text to the server, pull diagnostics, answer hover/gd.
    fn sync_lsp(&mut self) {
        for entry in self.panes.values_mut() {
            let Some(editor) = entry.editor.as_mut() else {
                continue;
            };
            let Some((client, language_id)) =
                Self::client_for(&mut self.lsp, &self.root, &editor.path)
            else {
                editor.lsp_op = None;
                continue;
            };
            if entry.lsp_synced_version == 0 {
                client.did_open(&editor.path, &editor.text(), language_id);
            } else if entry.lsp_synced_version != editor.version {
                client.did_change(&editor.path, &editor.text(), editor.version as i64);
            }
            entry.lsp_synced_version = editor.version;
            editor.diagnostics = client
                .diagnostics_for(&editor.path)
                .into_iter()
                .map(|d| (d.line, d.severity, d.message))
                .collect();
            match editor.lsp_op.take() {
                Some(LspOp::Hover) => {
                    let (line, col) = editor.cursor_line_col();
                    editor.message = client.hover(&editor.path, line, col);
                }
                Some(LspOp::Definition) => {
                    let (line, col) = editor.cursor_line_col();
                    if let Some((path, _line)) = client.definition(&editor.path, line, col) {
                        if path != editor.path {
                            editor.message = Some(format!("definition: {}", path.display()));
                        }
                    }
                }
                None => {}
            }
        }
    }

    /// Drop terminals whose child exited (`exit`, crash) and report their
    /// pane ids so the shell can close the pane / dock. Called every frame;
    /// `try_wait` is a cheap non-blocking check.
    pub fn reap_exited_terminals(&mut self) -> Vec<PaneId> {
        let mut dead: Vec<PaneId> = Vec::new();
        for (id, entry) in self.panes.iter_mut() {
            if let Some(term) = entry.term.as_mut() {
                if term.is_exited() {
                    entry.term = None;
                    dead.push(*id);
                }
            }
        }
        dead.sort_unstable();
        dead
    }

    pub fn term(&self, id: PaneId) -> Option<&TermPane> {
        self.panes.get(&id).and_then(|e| e.term.as_ref())
    }

    pub fn term_mut(&mut self, id: PaneId) -> Option<&mut TermPane> {
        self.panes.get_mut(&id).and_then(|e| e.term.as_mut())
    }

    pub fn editor(&self, id: PaneId) -> Option<&EditorPane> {
        self.panes.get(&id).and_then(|e| e.editor.as_ref())
    }

    pub fn editor_mut(&mut self, id: PaneId) -> Option<&mut EditorPane> {
        self.panes.get_mut(&id).and_then(|e| e.editor.as_mut())
    }

    pub fn agent(&self, id: PaneId) -> Option<&AgentPane> {
        self.panes.get(&id).and_then(|e| e.agent.as_ref())
    }

    pub fn agent_mut(&mut self, id: PaneId) -> Option<&mut AgentPane> {
        self.panes.get_mut(&id).and_then(|e| e.agent.as_mut())
    }

    pub fn search(&self, id: PaneId) -> Option<&SearchPane> {
        self.panes.get(&id).and_then(|e| e.search.as_ref())
    }

    pub fn search_mut(&mut self, id: PaneId) -> Option<&mut SearchPane> {
        self.panes.get_mut(&id).and_then(|e| e.search.as_mut())
    }

    pub fn lifeos(&self, id: PaneId) -> Option<&LifeOsPane> {
        self.panes.get(&id).and_then(|e| e.lifeos.as_ref())
    }

    pub fn lifeos_mut(&mut self, id: PaneId) -> Option<&mut LifeOsPane> {
        self.panes.get_mut(&id).and_then(|e| e.lifeos.as_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::PaneDesire;

    fn rect(w: u16, h: u16) -> Rect {
        Rect::new(0, 0, w, h)
    }

    #[test]
    fn sync_spawns_reaps_and_resizes_terminals() {
        let mut store = PaneStore::new(&std::env::temp_dir(), None);
        let desires = HashMap::new();
        store.sync(&[(0, rect(40, 20))], &desires);
        assert!(store.term(0).is_some());

        store.sync(&[(0, rect(40, 20)), (1, rect(40, 20))], &desires);
        assert!(store.term(1).is_some());

        store.sync(&[(1, rect(60, 12))], &desires);
        assert!(store.term(0).is_none(), "closed pane reaped");
        assert_eq!(store.term(1).unwrap().render_lines().len(), 11);
    }

    #[test]
    fn editor_desire_opens_file_and_terminal_survives_the_flip() {
        let file = std::env::temp_dir().join(format!("wb_store_{}.txt", std::process::id()));
        std::fs::write(&file, "hello\n").unwrap();
        let mut store = PaneStore::new(&std::env::temp_dir(), None);

        let mut desires = HashMap::new();
        store.sync(&[(0, rect(40, 20))], &desires);
        assert!(store.term(0).is_some());

        // Flip to editor: buffer opens, the pty is NOT killed.
        desires.insert(0, PaneDesire::Editor(file.clone()));
        store.sync(&[(0, rect(40, 20))], &desires);
        assert_eq!(store.editor(0).unwrap().text(), "hello\n");
        assert!(store.term(0).is_some(), "shell session survives");

        // Flip back: same terminal, editor kept for the next flip.
        desires.insert(0, PaneDesire::Terminal);
        store.sync(&[(0, rect(40, 20))], &desires);
        assert!(store.term(0).is_some() && store.editor(0).is_some());
        std::fs::remove_file(file).ok();
    }
}
