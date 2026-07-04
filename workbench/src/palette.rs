//! The universal command bar (DESIGN.md component 9): a centered
//! double-bordered modal with fuzzy-filtered actions, plus the configurable
//! keybinding layer that maps chords to the same command ids.

use crossterm::event::{KeyCode, KeyModifiers};
use std::collections::HashMap;

/// Everything invocable - from the palette, a keybinding, or both.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandId {
    SplitHorizontal,
    SplitVertical,
    ClosePane,
    FocusNext,
    FocusPrev,
    NewTab,
    NextTab,
    OpenPalette,
    ToggleEditor,
    TerminalHere,
    ToggleSidebar,
    ToggleDock,
    OpenFilePicker,
    OpenAgentPane,
    OpenSearchPane,
    OpenLifeOsPane,
    Quit,
}

#[derive(Clone, Debug)]
pub struct Command {
    pub id: CommandId,
    pub title: &'static str,
}

/// The built-in command registry, in palette display order.
pub fn commands() -> Vec<Command> {
    vec![
        Command {
            id: CommandId::SplitHorizontal,
            title: "pane: split right",
        },
        Command {
            id: CommandId::SplitVertical,
            title: "pane: split down",
        },
        Command {
            id: CommandId::ClosePane,
            title: "pane: close",
        },
        Command {
            id: CommandId::FocusNext,
            title: "pane: focus next",
        },
        Command {
            id: CommandId::FocusPrev,
            title: "pane: focus previous",
        },
        Command {
            id: CommandId::NewTab,
            title: "tab: new",
        },
        Command {
            id: CommandId::NextTab,
            title: "tab: next",
        },
        Command {
            id: CommandId::ToggleEditor,
            title: "pane: toggle editor/terminal",
        },
        Command {
            id: CommandId::TerminalHere,
            title: "pane: terminal here",
        },
        Command {
            id: CommandId::ToggleSidebar,
            title: "files: toggle sidebar",
        },
        Command {
            id: CommandId::ToggleDock,
            title: "terminal: toggle dock",
        },
        Command {
            id: CommandId::OpenFilePicker,
            title: "files: fuzzy picker",
        },
        Command {
            id: CommandId::OpenAgentPane,
            title: "agent: open pane",
        },
        Command {
            id: CommandId::OpenSearchPane,
            title: "search: recall (hybrid)",
        },
        Command {
            id: CommandId::OpenLifeOsPane,
            title: "life os: browse modules",
        },
        Command {
            id: CommandId::Quit,
            title: "workbench: quit",
        },
    ]
}

/// Case-insensitive subsequence fuzzy score. Higher is better; `None` means
/// no match. Consecutive hits and word-start hits score extra, so
/// "pane: split right" ranks above "pane: focus previous" for "spl".
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
        .filter_map(|(i, c)| fuzzy_score(query, c.title).map(|s| (s, i, c.clone())))
        .collect();
    // Stable order: score desc, then registry order.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, c)| c).collect()
}

/// Palette modal state. Pure - the app feeds it key events.
#[derive(Clone, Debug, Default)]
pub struct PaletteState {
    pub open: bool,
    pub query: String,
    pub selected: usize,
}

impl PaletteState {
    pub fn open() -> Self {
        Self {
            open: true,
            query: String::new(),
            selected: 0,
        }
    }

    pub fn matches(&self) -> Vec<Command> {
        filter(&self.query, &commands())
    }

    /// Apply a key; returns (new state, invoked command if Enter selected one).
    pub fn on_key(&self, code: KeyCode) -> (PaletteState, Option<CommandId>) {
        let mut next = self.clone();
        match code {
            KeyCode::Esc => next.open = false,
            KeyCode::Enter => {
                let picked = next.matches().get(next.selected).map(|c| c.id);
                next.open = false;
                return (next, picked);
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
        (next, None)
    }
}

/// A chord: key + modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

/// The configurable keybinding layer. Defaults follow the Zellij/tmux Alt
/// idiom; `bind` replaces or adds a chord at runtime.
#[derive(Clone, Debug)]
pub struct Keymap {
    bindings: HashMap<Chord, CommandId>,
}

impl Keymap {
    pub fn default_bindings() -> Self {
        let alt = KeyModifiers::ALT;
        let ctrl = KeyModifiers::CONTROL;
        let mut bindings = HashMap::new();
        bindings.insert(
            Chord {
                code: KeyCode::Char('s'),
                mods: alt,
            },
            CommandId::SplitHorizontal,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('v'),
                mods: alt,
            },
            CommandId::SplitVertical,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('x'),
                mods: alt,
            },
            CommandId::ClosePane,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Tab,
                mods: alt,
            },
            CommandId::FocusNext,
        );
        bindings.insert(
            Chord {
                code: KeyCode::BackTab,
                mods: alt,
            },
            CommandId::FocusPrev,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('n'),
                mods: alt,
            },
            CommandId::FocusNext,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('p'),
                mods: alt,
            },
            CommandId::FocusPrev,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('t'),
                mods: alt,
            },
            CommandId::NewTab,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char(']'),
                mods: alt,
            },
            CommandId::NextTab,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('k'),
                mods: ctrl,
            },
            CommandId::OpenPalette,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('e'),
                mods: alt,
            },
            CommandId::ToggleEditor,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('f'),
                mods: alt,
            },
            CommandId::ToggleSidebar,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('j'),
                mods: alt,
            },
            CommandId::ToggleDock,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('o'),
                mods: ctrl,
            },
            CommandId::OpenFilePicker,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('a'),
                mods: alt,
            },
            CommandId::OpenAgentPane,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('/'),
                mods: alt,
            },
            CommandId::OpenSearchPane,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('l'),
                mods: alt,
            },
            CommandId::OpenLifeOsPane,
        );
        bindings.insert(
            Chord {
                code: KeyCode::Char('q'),
                mods: ctrl,
            },
            CommandId::Quit,
        );
        Self { bindings }
    }

    pub fn lookup(&self, code: KeyCode, mods: KeyModifiers) -> Option<CommandId> {
        self.bindings.get(&Chord { code, mods }).copied()
    }

    /// Rebind (or add) a chord; returns a new keymap.
    pub fn bind(&self, chord: Chord, command: CommandId) -> Keymap {
        let mut bindings = self.bindings.clone();
        bindings.insert(chord, command);
        Keymap { bindings }
    }
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
        assert_eq!(all[0].id, CommandId::SplitHorizontal);
    }

    #[test]
    fn palette_typing_filters_and_enter_invokes() {
        let state = PaletteState::open();
        let (state, _) = state.on_key(KeyCode::Char('q'));
        let (state, _) = state.on_key(KeyCode::Char('u'));
        let (state, invoked) = state.on_key(KeyCode::Enter);
        assert_eq!(invoked, Some(CommandId::Quit));
        assert!(!state.open);
    }

    #[test]
    fn escape_closes_without_invoking() {
        let (state, invoked) = PaletteState::open().on_key(KeyCode::Esc);
        assert!(!state.open && invoked.is_none());
    }

    #[test]
    fn keymap_default_and_rebind() {
        let map = Keymap::default_bindings();
        assert_eq!(
            map.lookup(KeyCode::Char('s'), KeyModifiers::ALT),
            Some(CommandId::SplitHorizontal)
        );
        let map = map.bind(
            Chord {
                code: KeyCode::Char('o'),
                mods: KeyModifiers::CONTROL,
            },
            CommandId::OpenPalette,
        );
        assert_eq!(
            map.lookup(KeyCode::Char('o'), KeyModifiers::CONTROL),
            Some(CommandId::OpenPalette)
        );
    }
}
