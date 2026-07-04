//! Shared event dispatch between the two frontends (GPU window and `--tui`).
//! Both hosts produce crossterm-shaped events; this routes them either into
//! the focused pane (modal editor keys, VT bytes, agent/search input), the
//! mouse router (with the frame area for hit-testing), or the shell's own
//! keymap, returning the next shell state (immutably).

use crossterm::event::Event;
use ratatui::layout::Rect;

use crate::pane_store::PaneStore;
use crate::shell::{PaneDesire, Shell};

/// Route one event: mouse events hit-test against the chrome, pane-bound
/// keys go to the focused pane, everything else to the shell keymap.
/// Returns the (possibly unchanged) next shell.
pub fn dispatch(shell: Shell, panes: &mut PaneStore, ev: &Event, area: Rect) -> Shell {
    match ev {
        Event::Mouse(m) => crate::mouse::route(&shell, panes, m, area),
        _ if shell.forwards_to_pane(ev) => {
            forward_key(&shell, panes, ev);
            shell
        }
        _ => shell.on_event(ev),
    }
}

/// Route a pane-bound key: editors get modal keys, terminals get VT bytes.
fn forward_key(shell: &Shell, panes: &mut PaneStore, ev: &Event) {
    let Event::Key(key) = ev else {
        return;
    };
    let focused = shell.effective_focused_pane();
    match shell.focused_desire() {
        PaneDesire::Editor(_) => {
            if let Some(editor) = panes.editor_mut(focused) {
                let ctrl = key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL);
                editor.on_key(key.code, ctrl);
            }
        }
        PaneDesire::Agent => {
            if let Some(agent) = panes.agent_mut(focused) {
                agent.on_key(key.code);
            }
        }
        PaneDesire::Search => {
            if let Some(search) = panes.search_mut(focused) {
                search.on_key(key.code);
            }
        }
        PaneDesire::LifeOs => {
            if let Some(lifeos) = panes.lifeos_mut(focused) {
                lifeos.on_key(key.code);
            }
        }
        PaneDesire::Terminal => {
            if let Some(term) = panes.term_mut(focused) {
                term.send_key(key);
            }
        }
        // Welcome panes consume nothing (forwards_to_pane filters them).
        PaneDesire::Welcome => {}
    }
}
