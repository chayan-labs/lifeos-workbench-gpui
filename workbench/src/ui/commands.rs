//! The command registry + fuzzy matcher behind the palette.
//!
//! Ported from the legacy `palette.rs`, dropping the crossterm-coupled parts
//! (`Chord`/`Keymap`/`PaletteState::on_key`): gpui drives input via actions and
//! keystrokes, so only the pure registry + scorer survive here. Every command
//! the palette, the native menu, and the keymap can invoke is a [`CommandId`];
//! the single handler `WorkspaceView::run_command` gives each one behaviour.

/// Everything invocable - from the palette, a keybinding, or the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandId {
    SplitRight,
    SplitDown,
    ClosePane,
    FocusNext,
    FocusPrev,
    NewTab,
    NextTab,
    ToggleEditor,
    ToggleSidebar,
    ToggleDock,
    OpenFilePicker,
    FocusEditor,
    FocusTerminal,
    OpenAgentPane,
    OpenSearchPane,
    OpenLifeOsPane,
    Quit,
}

#[derive(Clone, Copy, Debug)]
pub struct Command {
    pub id: CommandId,
    pub title: &'static str,
}

/// The built-in command registry, in palette display order.
pub fn commands() -> Vec<Command> {
    use CommandId::*;
    [
        (SplitRight, "pane: split right"),
        (SplitDown, "pane: split down"),
        (ClosePane, "pane: close"),
        (FocusNext, "pane: focus next"),
        (FocusPrev, "pane: focus previous"),
        (NewTab, "tab: new"),
        (NextTab, "tab: next"),
        (ToggleEditor, "pane: toggle editor/terminal"),
        (FocusEditor, "go: editor"),
        (FocusTerminal, "go: terminal"),
        (ToggleSidebar, "files: toggle sidebar"),
        (ToggleDock, "terminal: toggle dock"),
        (OpenFilePicker, "files: fuzzy picker"),
        (OpenAgentPane, "agent: open pane"),
        (OpenSearchPane, "search: recall (hybrid)"),
        (OpenLifeOsPane, "life os: browse modules"),
        (Quit, "workbench: quit"),
    ]
    .into_iter()
    .map(|(id, title)| Command { id, title })
    .collect()
}

/// Case-insensitive subsequence fuzzy score. Higher is better; `None` means no
/// match. Consecutive hits and word-start hits score extra, so "pane: split
/// right" ranks above "pane: focus previous" for "spl".
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let cand: Vec<char> = candidate.to_lowercase().chars().collect();
    let mut score = 0;
    let mut pos = 0usize;
    let mut last_hit: Option<usize> = None;
    for qc in query.to_lowercase().chars() {
        let found = cand[pos..].iter().position(|c| *c == qc)? + pos;
        score += 1;
        if last_hit == Some(found.wrapping_sub(1)) {
            score += 2; // consecutive run
        }
        if found == 0 || matches!(cand.get(found.wrapping_sub(1)), Some(' ') | Some(':')) {
            score += 3; // word start
        }
        last_hit = Some(found);
        pos = found + 1;
    }
    Some(score)
}

/// Filter + rank the registry against a query.
pub fn filter(query: &str, registry: &[Command]) -> Vec<Command> {
    let mut scored: Vec<(i32, usize, Command)> = registry
        .iter()
        .enumerate()
        .filter_map(|(i, c)| fuzzy_score(query, c.title).map(|s| (s, i, *c)))
        .collect();
    // Stable order: score desc, then registry order.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_matches_subsequences_and_rejects_non_matches() {
        assert!(fuzzy_score("spl", "pane: split right").is_some());
        assert!(fuzzy_score("zzz", "pane: split right").is_none());
    }

    #[test]
    fn word_start_and_consecutive_runs_rank_higher() {
        let results = filter("split", &commands());
        assert_eq!(results[0].title, "pane: split right");
        let quit = filter("quit", &commands());
        assert_eq!(quit[0].id, CommandId::Quit);
    }

    #[test]
    fn empty_query_returns_full_registry_in_order() {
        let all = filter("", &commands());
        assert_eq!(all.len(), commands().len());
        assert_eq!(all[0].id, CommandId::SplitRight);
    }
}
