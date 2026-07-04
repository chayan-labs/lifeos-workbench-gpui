//! Mouse routing above the Backend seam: crossterm-shaped mouse events (from
//! the GPU window's pixel→cell translation or the terminal's mouse capture)
//! are hit-tested against the workspace chrome and delivered as focus
//! changes, cursor placement, scrolling, and modal activation. All cell
//! coordinates, so it behaves identically in both frontends.

use crate::pane_store::PaneStore;
use crate::shell::{PaneDesire, Shell};
use crate::workspace::{self, ChromeRects, Region, TabHit, DOCK_PANE};
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

fn contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

/// Route one mouse event, returning the next shell state.
pub fn route(shell: &Shell, panes: &mut PaneStore, m: &MouseEvent, area: Rect) -> Shell {
    if shell.palette.open || shell.picker.is_some() {
        return modal_mouse(shell, m, area);
    }
    let Some(cr) = shell.chrome_rects(area) else {
        return shell.clone();
    };
    match m.kind {
        MouseEventKind::Down(MouseButton::Left) => click(shell, panes, m, &cr, false),
        MouseEventKind::Drag(MouseButton::Left) => click(shell, panes, m, &cr, true),
        MouseEventKind::ScrollUp => wheel(shell, panes, m, &cr, false),
        MouseEventKind::ScrollDown => wheel(shell, panes, m, &cr, true),
        _ => shell.clone(),
    }
}

/// Palette / picker: click a row to activate it, click outside to dismiss,
/// wheel to move the selection.
fn modal_mouse(shell: &Shell, m: &MouseEvent, area: Rect) -> Shell {
    let modal = shell.modal_rect(area);
    match m.kind {
        MouseEventKind::ScrollUp => modal_key(shell, KeyCode::Up),
        MouseEventKind::ScrollDown => modal_key(shell, KeyCode::Down),
        MouseEventKind::Down(MouseButton::Left) => {
            if !contains(modal, m.column, m.row) {
                return modal_key(shell, KeyCode::Esc);
            }
            let Some(row) = m.row.checked_sub(modal.y + 1) else {
                return shell.clone();
            };
            if row as usize >= modal.height.saturating_sub(2) as usize {
                return shell.clone();
            }
            modal_pick(shell, row as usize)
        }
        _ => shell.clone(),
    }
}

/// Feed one key into whichever modal is open (they share list semantics).
fn modal_key(shell: &Shell, code: KeyCode) -> Shell {
    let mut next = shell.clone();
    if shell.palette.open {
        let (palette, invoked) = shell.palette.on_key(code);
        next.palette = palette;
        if let Some(cmd) = invoked {
            return next.run_command(cmd);
        }
        return next;
    }
    if let Some(picker) = &shell.picker {
        let (picker, action) = picker.on_key(code);
        next.picker = Some(picker);
        match action {
            crate::file_tree::PickerAction::Close => next.picker = None,
            crate::file_tree::PickerAction::OpenFile(path) => return next.open_in_focused(path),
            crate::file_tree::PickerAction::None => {}
        }
    }
    next
}

/// Select row `idx` in the open modal and activate it (single-click open).
fn modal_pick(shell: &Shell, idx: usize) -> Shell {
    let mut next = shell.clone();
    if shell.palette.open {
        if idx >= shell.palette.matches().len() {
            return next;
        }
        next.palette.selected = idx;
        return modal_key(&next, KeyCode::Enter);
    }
    if let Some(picker) = &shell.picker {
        if idx >= picker.matches().len() {
            return next;
        }
        if let Some(p) = &mut next.picker {
            p.selected = idx;
        }
        return modal_key(&next, KeyCode::Enter);
    }
    next
}

fn click(
    shell: &Shell,
    panes: &mut PaneStore,
    m: &MouseEvent,
    cr: &ChromeRects,
    drag: bool,
) -> Shell {
    let (col, row) = (m.column, m.row);
    if contains(cr.tab_bar, col, row) {
        if drag {
            return shell.clone();
        }
        return match workspace::tab_hit(&shell.layout, col - cr.tab_bar.x) {
            Some(TabHit::Tab(i)) => Shell {
                layout: shell.layout.switch_tab(i),
                ..shell.clone()
            },
            Some(TabHit::NewTab) => shell.run_command(crate::palette::CommandId::NewTab),
            None => shell.clone(),
        };
    }
    if let Some(sb) = cr.sidebar {
        if contains(sb, col, row) {
            return sidebar_click(shell, col, row, sb, drag);
        }
    }
    if let Some(dock) = cr.dock {
        if contains(dock, col, row) {
            // The dock header's × hides the dock (its session survives).
            if !drag && contains(workspace::close_button(dock), col, row) {
                return shell.run_command(crate::palette::CommandId::ToggleDock);
            }
            let mut next = shell.clone();
            next.chrome.focus = Region::Dock;
            return next;
        }
    }
    for (pane, rect) in shell.layout.tab().root.rects(cr.center) {
        if !contains(rect, col, row) {
            continue;
        }
        if !drag && contains(workspace::close_button(rect), col, row) {
            return shell.close_center_pane(pane);
        }
        let mut next = shell.clone();
        next.chrome.focus = Region::Center;
        next.layout = shell.layout.focus_pane(pane);
        // Editors: place the cursor at the clicked cell (drag moves it too).
        match shell.desires.get(&pane) {
            Some(PaneDesire::Editor(_)) => {
                if let Some(editor) = panes.editor_mut(pane) {
                    let inner = workspace::pane_content(rect);
                    if contains(inner, col, row) {
                        let content_col =
                            (col - inner.x).saturating_sub(editor.gutter_cols() as u16);
                        editor.on_click((row - inner.y) as usize, content_col as usize);
                    }
                }
            }
            // Life OS lists: click a row to select + activate it.
            Some(PaneDesire::LifeOs) if !drag => {
                if let Some(lifeos) = panes.lifeos_mut(pane) {
                    let inner = workspace::pane_content(rect);
                    if contains(inner, col, row) {
                        lifeos.on_click((row - inner.y) as usize);
                    }
                }
            }
            _ => {}
        }
        return next;
    }
    shell.clone()
}

/// Sidebar click: select the hit row and activate it (expand / open).
fn sidebar_click(shell: &Shell, col: u16, row: u16, sb: Rect, drag: bool) -> Shell {
    let mut next = shell.clone();
    next.chrome.focus = Region::Sidebar;
    let Some(tree) = &shell.tree else {
        return next;
    };
    // Rows start below the flat panel's FILES header.
    let inner = workspace::pane_content(sb);
    if drag || !contains(inner, col, row) {
        return next;
    }
    let height = inner.height as usize;
    let idx = workspace::scroll_offset(tree.selected, height) + (row - inner.y) as usize;
    if idx >= tree.rows().len() {
        return next;
    }
    let mut tree = tree.clone();
    tree.selected = idx;
    let (tree, action) = tree.on_key(KeyCode::Enter);
    next.tree = Some(tree);
    match action {
        crate::file_tree::TreeAction::OpenFile(path) => next.open_in_focused(path),
        _ => next,
    }
}

/// Wheel scrolls whatever is under the cursor - no focus change (Zed feel).
fn wheel(
    shell: &Shell,
    panes: &mut PaneStore,
    m: &MouseEvent,
    cr: &ChromeRects,
    down: bool,
) -> Shell {
    let (col, row) = (m.column, m.row);
    if let Some(sb) = cr.sidebar {
        if contains(sb, col, row) {
            if let Some(tree) = &shell.tree {
                let mut tree = tree.clone();
                let max = tree.rows().len().saturating_sub(1);
                tree.selected = if down {
                    (tree.selected + 1).min(max)
                } else {
                    tree.selected.saturating_sub(1)
                };
                return Shell {
                    tree: Some(tree),
                    ..shell.clone()
                };
            }
            return shell.clone();
        }
    }
    if let Some(dock) = cr.dock {
        if contains(dock, col, row) {
            if let Some(term) = panes.term_mut(DOCK_PANE) {
                term.on_scroll(down);
            }
            return shell.clone();
        }
    }
    for (pane, rect) in shell.layout.tab().root.rects(cr.center) {
        if !contains(rect, col, row) {
            continue;
        }
        match shell.desires.get(&pane) {
            Some(PaneDesire::Editor(_)) => {
                if let Some(editor) = panes.editor_mut(pane) {
                    editor.on_scroll(down);
                }
            }
            Some(PaneDesire::Agent) => {
                if let Some(agent) = panes.agent_mut(pane) {
                    agent.on_key(if down { KeyCode::Down } else { KeyCode::Up });
                }
            }
            Some(PaneDesire::Search) => {
                if let Some(search) = panes.search_mut(pane) {
                    search.on_key(if down { KeyCode::Down } else { KeyCode::Up });
                }
            }
            Some(PaneDesire::LifeOs) => {
                if let Some(lifeos) = panes.lifeos_mut(pane) {
                    lifeos.on_key(if down { KeyCode::Down } else { KeyCode::Up });
                }
            }
            Some(PaneDesire::Welcome) => {}
            _ => {
                if let Some(term) = panes.term_mut(pane) {
                    term.on_scroll(down);
                }
            }
        }
        return shell.clone();
    }
    shell.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_tree::FileTree;
    use crate::theme::{ColorSupport, Theme};
    use crossterm::event::KeyModifiers;
    use std::path::PathBuf;

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 120,
        height: 40,
    };

    fn shell() -> Shell {
        Shell::new(Theme::new(ColorSupport::TrueColor), "test-ws".into())
    }

    fn panes() -> PaneStore {
        PaneStore::new(&std::env::current_dir().unwrap(), None)
    }

    fn down(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn wheel_ev(col: u16, row: u16, down: bool) -> MouseEvent {
        MouseEvent {
            kind: if down {
                MouseEventKind::ScrollDown
            } else {
                MouseEventKind::ScrollUp
            },
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn center_of(r: Rect) -> (u16, u16) {
        (r.x + r.width / 2, r.y + r.height / 2)
    }

    #[test]
    fn clicking_a_center_pane_focuses_it() {
        let s = shell()
            .run_command(crate::palette::CommandId::SplitHorizontal)
            .run_command(crate::palette::CommandId::ToggleDock); // focus leaves center
        let cr = s.chrome_rects(AREA).unwrap();
        let rects = s.layout.tab().root.rects(cr.center);
        let (col, row) = center_of(rects[0].1);
        let next = route(&s, &mut panes(), &down(col, row), AREA);
        assert_eq!(next.chrome.focus, Region::Center);
        assert_eq!(next.layout.tab().focused, rects[0].0);
    }

    #[test]
    fn clicking_the_dock_focuses_the_terminal() {
        let s = shell();
        let cr = s.chrome_rects(AREA).unwrap();
        let (col, row) = center_of(cr.dock.unwrap());
        let next = route(&s, &mut panes(), &down(col, row), AREA);
        assert_eq!(next.chrome.focus, Region::Dock);
        assert_eq!(next.effective_focused_pane(), DOCK_PANE);
    }

    #[test]
    fn tab_bar_clicks_switch_and_create_tabs() {
        let s = shell().run_command(crate::palette::CommandId::NewTab);
        assert_eq!(s.layout.active_tab, 1);
        // ` 1 ` starts at col 0; click it.
        let next = route(&s, &mut panes(), &down(1, 0), AREA);
        assert_eq!(next.layout.active_tab, 0);
        // ` 1  2  + `: '+' segment starts at col 6.
        let next = route(&s, &mut panes(), &down(7, 0), AREA);
        assert_eq!(next.layout.tabs.len(), 3);
    }

    #[test]
    fn sidebar_click_opens_a_file_and_wheel_moves_selection() {
        let root = std::env::temp_dir().join(format!("wb_mouse_{}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("README.md"), "hi").unwrap();
        std::fs::write(root.join("src/a.rs"), "fn a() {}").unwrap();
        let mut s = shell();
        s.tree = Some(FileTree::open(&root));
        let cr = s.chrome_rects(AREA).unwrap();
        let sb = cr.sidebar.unwrap();

        // Wheel down moves the selection without opening anything.
        let s2 = route(&s, &mut panes(), &wheel_ev(sb.x + 2, sb.y + 2, true), AREA);
        assert_eq!(s2.tree.as_ref().unwrap().selected, 1);

        // Rows: [src (dir), README.md]. Click row 1 opens the file.
        let next = route(&s, &mut panes(), &down(sb.x + 2, sb.y + 2), AREA);
        let PaneDesire::Editor(path) = next.focused_desire() else {
            panic!("expected editor desire, got {:?}", next.focused_desire());
        };
        assert_eq!(path, root.join("README.md"));
        assert_eq!(next.chrome.focus, Region::Center);

        // Click row 0 (the dir) expands it instead.
        let next = route(&s, &mut panes(), &down(sb.x + 2, sb.y + 1), AREA);
        assert!(next
            .tree
            .as_ref()
            .unwrap()
            .rows()
            .iter()
            .any(|r| r.path.ends_with("a.rs")));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn header_close_button_closes_the_pane_and_hides_the_dock() {
        let s = shell().run_command(crate::palette::CommandId::SplitHorizontal);
        let cr = s.chrome_rects(AREA).unwrap();
        let rects = s.layout.tab().root.rects(cr.center);
        assert_eq!(rects.len(), 2);
        // Click the × in the second pane's header.
        let close = crate::workspace::close_button(rects[1].1);
        let next = route(&s, &mut panes(), &down(close.x + 1, close.y), AREA);
        assert_eq!(next.layout.tab().root.panes().len(), 1);
        // The dock's × hides the dock without killing the pane entry.
        let dock = cr.dock.unwrap();
        let close = crate::workspace::close_button(dock);
        let next = route(&s, &mut panes(), &down(close.x + 1, close.y), AREA);
        assert!(!next.chrome.dock_open);
        assert!(next.pane_rects(AREA).iter().any(|(id, _)| *id == DOCK_PANE));
    }

    #[test]
    fn palette_click_activates_a_row_and_outside_click_dismisses() {
        let s = shell().run_command(crate::palette::CommandId::OpenPalette);
        let modal = s.modal_rect(AREA);
        // Row 0 = first registry command = "pane: split right".
        let next = route(&s, &mut panes(), &down(modal.x + 2, modal.y + 1), AREA);
        assert!(!next.palette.open);
        assert_eq!(next.layout.tab().root.panes().len(), 2);
        // Clicking outside closes without invoking.
        let next = route(&s, &mut panes(), &down(0, AREA.height - 1), AREA);
        assert!(!next.palette.open);
        assert_eq!(next.layout.tab().root.panes().len(), 1);
    }

    #[test]
    fn editor_click_places_the_cursor() {
        // .md avoids an LSP spawn; skip the dock rect to avoid a test pty.
        let file = std::env::temp_dir().join(format!("wb_mouse_ed_{}.md", std::process::id()));
        std::fs::write(&file, "abc\ndef\nghi\n").unwrap();
        let s = shell().open_in_focused(PathBuf::from(&file));
        let mut store = panes();
        let rects: Vec<_> = s
            .pane_rects(AREA)
            .into_iter()
            .filter(|(id, _)| *id != DOCK_PANE)
            .collect();
        store.sync(&rects, &s.desires);
        let cr = s.chrome_rects(AREA).unwrap();
        let rect = s.layout.tab().root.rects(cr.center)[0].1;
        let gutter = store.editor(0).unwrap().gutter_cols() as u16;
        // Click line 2 (row offset 1), column 1 of content (below header).
        let ev = down(rect.x + gutter + 1, rect.y + 1 + 1);
        route(&s, &mut store, &ev, AREA);
        assert_eq!(store.editor(0).unwrap().cursor_line_col(), (1, 1));
        std::fs::remove_file(file).ok();
    }
}
