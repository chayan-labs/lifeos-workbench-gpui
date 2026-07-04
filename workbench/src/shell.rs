//! The interactive shell: renders the IDE workspace chrome (tab bar, file
//! sidebar, editor center, terminal dock, statusline), the command palette,
//! and the fuzzy picker with the Terminal Brutalism theme, and routes key
//! events to chords, modals, the sidebar, or the focused pane. Pane content
//! lives in the `PaneStore`; everything here is cloneable value-state.

use crate::file_tree::{FileTree, PickerAction, PickerState, TreeAction};
use crate::layout::{Layout, PaneId, SplitDir};
use crate::palette::{CommandId, Keymap, PaletteState};
use crate::pane_store::PaneStore;
use crate::theme::{self, StatuslineState, Theme};
use crate::workspace::{self, Chrome, Region, DOCK_PANE};
use crossterm::event::{Event, KeyEvent, KeyEventKind};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;
use std::collections::HashMap;
use std::path::PathBuf;

/// What the pane should show; the `PaneStore` reconciles toward this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaneDesire {
    /// Empty editor surface: keybinding hints until something opens.
    Welcome,
    Terminal,
    Editor(PathBuf),
    Agent,
    Search,
    /// The Life OS module browser (manifests → views → rendered entities).
    LifeOs,
}

/// Whole-shell state. Cloned-and-replaced per event (immutable convention).
#[derive(Clone)]
pub struct Shell {
    pub layout: Layout,
    pub palette: PaletteState,
    pub keymap: Keymap,
    pub theme: Theme,
    pub status: StatuslineState,
    pub running: bool,
    pub desires: HashMap<PaneId, PaneDesire>,
    /// The persistent file sidebar; `None` = collapsed.
    pub tree: Option<FileTree>,
    pub picker: Option<PickerState>,
    pub chrome: Chrome,
}

impl Shell {
    pub fn new(theme: Theme, workspace: String) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "?".into());
        let mut desires = HashMap::new();
        desires.insert(0, PaneDesire::Welcome);
        desires.insert(DOCK_PANE, PaneDesire::Terminal);
        Self {
            layout: Layout::new(),
            palette: PaletteState::default(),
            keymap: Keymap::default_bindings(),
            theme,
            status: StatuslineState {
                mode: "SHELL".into(),
                cwd: cwd.clone(),
                workspace,
                ..Default::default()
            },
            running: true,
            desires,
            tree: Some(FileTree::open(&PathBuf::from(cwd))),
            picker: None,
            chrome: Chrome::default(),
        }
    }

    fn cwd_path(&self) -> PathBuf {
        PathBuf::from(&self.status.cwd)
    }

    /// The pane that keyboard input lands in: the dock when it has focus,
    /// otherwise the layout's focused center pane.
    pub fn effective_focused_pane(&self) -> PaneId {
        match self.chrome.focus {
            Region::Dock => DOCK_PANE,
            _ => self.layout.tab().focused,
        }
    }

    pub fn focused_desire(&self) -> PaneDesire {
        self.desires
            .get(&self.effective_focused_pane())
            .cloned()
            .unwrap_or(PaneDesire::Welcome)
    }

    fn any_modal_open(&self) -> bool {
        self.palette.open || self.picker.is_some()
    }

    /// Apply one terminal event, returning the next state.
    pub fn on_event(&self, event: &Event) -> Shell {
        let Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        }) = event
        else {
            return self.clone();
        };
        if *kind != KeyEventKind::Press {
            return self.clone();
        }
        if self.palette.open {
            let (palette, invoked) = self.palette.on_key(*code);
            let next = Shell {
                palette,
                ..self.clone()
            };
            return match invoked {
                Some(cmd) => next.run_command(cmd),
                None => next,
            };
        }
        if let Some(picker) = &self.picker {
            let (picker, action) = picker.on_key(*code);
            let mut next = Shell {
                picker: Some(picker),
                ..self.clone()
            };
            match action {
                PickerAction::Close => next.picker = None,
                PickerAction::OpenFile(path) => return next.open_in_focused(path),
                PickerAction::None => {}
            }
            return next;
        }
        if let Some(cmd) = self.keymap.lookup(*code, *modifiers) {
            return self.run_command(cmd);
        }
        // The sidebar owns plain keys while focused (j/k/enter navigation).
        if self.chrome.focus == Region::Sidebar {
            if let Some(tree) = &self.tree {
                let (tree, action) = tree.on_key(*code);
                let mut next = Shell {
                    tree: Some(tree),
                    ..self.clone()
                };
                match action {
                    TreeAction::Close => next.chrome.focus = Region::Center,
                    TreeAction::OpenFile(path) => return next.open_in_focused(path),
                    TreeAction::None => {}
                }
                return next;
            }
        }
        self.clone()
    }

    /// Open a file in the focused center pane's editor, closing any modal
    /// and pulling focus into the editor (Zed behavior). Public so the
    /// window host can route drag-and-dropped files here.
    pub fn open_in_focused(&self, path: PathBuf) -> Shell {
        let mut next = self.clone();
        next.picker = None;
        next.chrome.focus = Region::Center;
        next.desires
            .insert(self.layout.tab().focused, PaneDesire::Editor(path));
        next
    }

    /// Close a center pane (mouse ×, alt+x, or its shell exiting). The
    /// last pane never leaves an empty frame: it reverts to the welcome
    /// surface if it held content, and quits only when already welcome.
    pub fn close_center_pane(&self, pane: PaneId) -> Shell {
        let mut next = self.clone();
        match self.layout.close_pane(pane) {
            Some(layout) => {
                next.layout = layout;
                next.desires.remove(&pane);
            }
            None => {
                if matches!(self.desires.get(&pane), Some(PaneDesire::Welcome) | None) {
                    next.running = false;
                } else {
                    next.desires.insert(pane, PaneDesire::Welcome);
                }
            }
        }
        next
    }

    /// A terminal pane's shell exited (`exit`): the dock closes (a fresh
    /// shell spawns next time it opens), a center terminal pane closes like
    /// a clicked ×. Panes in background tabs respawn on revisit instead.
    pub fn on_pane_exit(&self, pane: PaneId) -> Shell {
        if pane == DOCK_PANE {
            let mut next = self.clone();
            next.chrome.dock_open = false;
            next.desires.insert(DOCK_PANE, PaneDesire::Welcome);
            if next.chrome.focus == Region::Dock {
                next.chrome.focus = Region::Center;
            }
            return next;
        }
        let is_terminal = matches!(self.desires.get(&pane), Some(PaneDesire::Terminal));
        if !is_terminal || !self.layout.tab().root.panes().contains(&pane) {
            return self.clone();
        }
        self.close_center_pane(pane)
    }

    /// True when a key press belongs to the focused pane (terminal, editor,
    /// dock, ...) rather than a chord, an open modal, or the sidebar.
    pub fn forwards_to_pane(&self, event: &Event) -> bool {
        let Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        }) = event
        else {
            return false;
        };
        *kind == KeyEventKind::Press
            && !self.any_modal_open()
            && self.chrome.focus != Region::Sidebar
            && self.focused_desire() != PaneDesire::Welcome
            && self.keymap.lookup(*code, *modifiers).is_none()
    }

    pub fn run_command(&self, cmd: CommandId) -> Shell {
        let mut next = self.clone();
        match cmd {
            CommandId::SplitHorizontal => {
                let (layout, pane) = self.layout.split_focused(SplitDir::Horizontal);
                next.layout = layout;
                next.desires.insert(pane, PaneDesire::Welcome);
                next.chrome.focus = Region::Center;
            }
            CommandId::SplitVertical => {
                let (layout, pane) = self.layout.split_focused(SplitDir::Vertical);
                next.layout = layout;
                next.desires.insert(pane, PaneDesire::Welcome);
                next.chrome.focus = Region::Center;
            }
            CommandId::ClosePane => match self.chrome.focus {
                Region::Dock => {
                    next.chrome.dock_open = false;
                    next.chrome.focus = Region::Center;
                }
                _ => return self.close_center_pane(self.layout.tab().focused),
            },
            CommandId::FocusNext | CommandId::FocusPrev if self.chrome.focus != Region::Center => {
                next.chrome.focus = Region::Center;
            }
            CommandId::FocusNext => next.layout = self.layout.focus_next(),
            CommandId::FocusPrev => next.layout = self.layout.focus_prev(),
            CommandId::NewTab => {
                let (layout, pane) = self.layout.new_tab();
                next.layout = layout;
                next.desires.insert(pane, PaneDesire::Welcome);
                next.chrome.focus = Region::Center;
            }
            CommandId::NextTab => next.layout = self.layout.next_tab(),
            CommandId::OpenPalette => next.palette = PaletteState::open(),
            CommandId::ToggleEditor => {
                let focused = self.layout.tab().focused;
                next.chrome.focus = Region::Center;
                match self.desires.get(&focused) {
                    Some(PaneDesire::Editor(_)) => {
                        next.desires.insert(focused, PaneDesire::Terminal);
                    }
                    // No file yet: the picker chooses one, sharing the cwd.
                    _ => next.picker = Some(PickerState::open(&self.cwd_path())),
                }
            }
            CommandId::TerminalHere => {
                next.desires
                    .insert(self.layout.tab().focused, PaneDesire::Terminal);
                next.chrome.focus = Region::Center;
            }
            CommandId::OpenAgentPane => {
                next.desires
                    .insert(self.layout.tab().focused, PaneDesire::Agent);
                next.chrome.focus = Region::Center;
            }
            CommandId::OpenSearchPane => {
                next.desires
                    .insert(self.layout.tab().focused, PaneDesire::Search);
                next.chrome.focus = Region::Center;
            }
            CommandId::OpenLifeOsPane => {
                next.desires
                    .insert(self.layout.tab().focused, PaneDesire::LifeOs);
                next.chrome.focus = Region::Center;
            }
            CommandId::ToggleSidebar => match &self.tree {
                Some(_) => {
                    next.tree = None;
                    if next.chrome.focus == Region::Sidebar {
                        next.chrome.focus = Region::Center;
                    }
                }
                None => {
                    next.tree = Some(FileTree::open(&self.cwd_path()));
                    next.chrome.focus = Region::Sidebar;
                }
            },
            CommandId::ToggleDock => {
                next.chrome.dock_open = !self.chrome.dock_open;
                next.chrome.focus = if next.chrome.dock_open {
                    Region::Dock
                } else {
                    Region::Center
                };
                // Re-arm the dock's shell (its desire drops to Welcome when
                // the previous shell exits via `exit`).
                if next.chrome.dock_open {
                    next.desires.insert(DOCK_PANE, PaneDesire::Terminal);
                }
            }
            CommandId::OpenFilePicker => next.picker = Some(PickerState::open(&self.cwd_path())),
            CommandId::Quit => next.running = false,
        }
        next
    }

    /// The chrome rectangles for a frame (sidebar/dock reflect actual state).
    pub fn chrome_rects(&self, area: Rect) -> Option<workspace::ChromeRects> {
        workspace::chrome_rects(area, self.tree.is_some(), self.chrome.dock_open)
    }

    /// The pane rectangles the `PaneStore` reconciles against: the active
    /// tab's center panes plus the terminal dock. The dock entry is present
    /// even while hidden so its shell session survives toggling.
    pub fn pane_rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        let Some(cr) = self.chrome_rects(area) else {
            return Vec::new();
        };
        let mut rects = self.layout.tab().root.rects(cr.center);
        let dock = cr
            .dock
            .or_else(|| workspace::chrome_rects(area, self.tree.is_some(), true)?.dock);
        if let Some(rect) = dock {
            rects.push((DOCK_PANE, rect));
        }
        rects
    }

    pub fn draw(&self, frame: &mut Frame, panes: &mut PaneStore) {
        let area = frame.area();
        let Some(cr) = self.chrome_rects(area) else {
            return;
        };

        self.draw_tab_bar(frame, cr.tab_bar);
        if let (Some(tree), Some(rect)) = (&self.tree, cr.sidebar) {
            self.draw_sidebar(frame, tree, rect);
        }

        let tab = self.layout.tab();
        let center_right = cr.center.x + cr.center.width;
        for (pane, rect) in tab.root.rects(cr.center) {
            let focused = self.chrome.focus == Region::Center && pane == tab.focused;
            let separator = rect.x + rect.width < center_right;
            self.draw_pane(frame, panes, pane, rect, focused, separator);
        }
        if let Some(rect) = cr.dock {
            self.draw_pane(
                frame,
                panes,
                DOCK_PANE,
                rect,
                self.chrome.focus == Region::Dock,
                false,
            );
        }

        let mut status = self.status.clone();
        status.mode = match self.chrome.focus {
            Region::Sidebar => "FILES".into(),
            Region::Dock => "TERMINAL".into(),
            Region::Center => match self.focused_desire() {
                PaneDesire::Editor(_) => panes
                    .editor(tab.focused)
                    .map(|e| e.status())
                    .unwrap_or_else(|| "EDITOR".into()),
                PaneDesire::Terminal => "TERMINAL".into(),
                PaneDesire::Agent => "AGENT".into(),
                PaneDesire::Search => "RECALL".into(),
                PaneDesire::LifeOs => "LIFE OS".into(),
                PaneDesire::Welcome => "SHELL".into(),
            },
        };
        if let Some(agent) = panes.agent(tab.focused) {
            status.agent = agent.status();
        }
        frame.render_widget(
            Paragraph::new(theme::statusline(&self.theme, &status)).style(self.theme.panel_bg()),
            cr.status,
        );

        if self.palette.open {
            self.draw_palette(frame, area);
        }
        if self.picker.is_some() {
            self.draw_files_modal(frame, area);
        }
    }

    fn draw_tab_bar(&self, frame: &mut Frame, rect: Rect) {
        let spans: Vec<Span> = workspace::tab_bar_items(&self.layout)
            .into_iter()
            .map(|(label, hit)| {
                let style = match hit {
                    workspace::TabHit::Tab(i) if i == self.layout.active_tab => {
                        self.theme.tab_active()
                    }
                    _ => self.theme.tab_inactive(),
                };
                Span::styled(label, style)
            })
            .collect();
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(self.theme.panel_bg()),
            rect,
        );
    }

    fn draw_sidebar(&self, frame: &mut Frame, tree: &FileTree, rect: Rect) {
        let focused = self.chrome.focus == Region::Sidebar;
        // Flat Zed-style panel: shaded fill, FILES header, no box border.
        frame.render_widget(Block::default().style(self.theme.panel_bg()), rect);
        let header_style = if focused {
            Style::default()
                .fg(theme::ACCENT.resolve(self.theme.support))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        frame.render_widget(
            Paragraph::new(Line::styled(" FILES", header_style)).style(self.theme.panel_bg()),
            workspace::pane_header(rect),
        );
        let list_rect = workspace::pane_content(rect);
        let height = list_rect.height as usize;
        let scroll = workspace::scroll_offset(tree.selected, height);
        let rows: Vec<ListItem> = tree
            .rows()
            .iter()
            .enumerate()
            .skip(scroll)
            .take(height)
            .map(|(i, r)| {
                let name = r.path.file_name().map(|n| n.to_string_lossy().to_string());
                let glyph = match (r.is_dir, r.expanded) {
                    (true, true) => "▾ ",
                    (true, false) => "▸ ",
                    _ => "  ",
                };
                let label = format!(
                    " {}{glyph}{}",
                    "  ".repeat(r.depth),
                    name.unwrap_or_default()
                );
                let style = if i == tree.selected {
                    self.theme.active_item()
                } else {
                    Style::default().fg(theme::FG.resolve(self.theme.support))
                };
                ListItem::new(label).style(style)
            })
            .collect();
        frame.render_widget(List::new(rows).style(self.theme.panel_bg()), list_rect);
    }

    /// A pane's kind dot color: instant visual identification in the header.
    fn pane_dot(&self, desire: &PaneDesire) -> Style {
        let color = match desire {
            PaneDesire::Editor(_) => theme::PRIMARY,
            PaneDesire::Terminal => theme::SUCCESS,
            PaneDesire::Agent => theme::ACCENT,
            PaneDesire::Search => theme::PRIMARY,
            PaneDesire::LifeOs => theme::PRIMARY,
            PaneDesire::Welcome => theme::FG_DIM,
        };
        Style::default().fg(color.resolve(self.theme.support))
    }

    fn draw_pane(
        &self,
        frame: &mut Frame,
        panes: &mut PaneStore,
        pane: PaneId,
        rect: Rect,
        focused: bool,
        separator: bool,
    ) {
        if rect.height < 2 || rect.width < 5 {
            return;
        }
        let desire = self
            .desires
            .get(&pane)
            .cloned()
            .unwrap_or(PaneDesire::Terminal);
        let title = match &desire {
            PaneDesire::Editor(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            PaneDesire::Terminal => "terminal".to_string(),
            PaneDesire::Agent => "agent".to_string(),
            PaneDesire::Search => "recall".to_string(),
            PaneDesire::LifeOs => "life os".to_string(),
            PaneDesire::Welcome => "welcome".to_string(),
        };
        // Zed-style flat pane: one header row (dot + title + ×), content
        // below, no box borders.
        let header_style = if focused {
            self.theme.header_focused()
        } else {
            self.theme.header_unfocused()
        };
        let header_line = Line::from(vec![
            Span::styled(" ● ".to_string(), self.pane_dot(&desire)),
            Span::raw(title),
        ]);
        frame.render_widget(
            Paragraph::new(header_line).style(header_style),
            workspace::pane_header(rect),
        );
        frame.render_widget(
            Paragraph::new(" × ").style(header_style),
            workspace::close_button(rect),
        );

        let content = workspace::pane_content(rect);
        let inner_height = content.height as usize;
        let widget = match &desire {
            PaneDesire::Editor(_) => match panes.editor_mut(pane) {
                Some(editor) => Paragraph::new(editor.render_lines(inner_height)),
                None => Paragraph::new("opening…").style(self.theme.muted()),
            },
            PaneDesire::Terminal => match panes.term(pane) {
                Some(term) => Paragraph::new(term.render_lines()),
                None => Paragraph::new("no shell - ctrl-k for commands").style(self.theme.muted()),
            },
            PaneDesire::Agent => match panes.agent(pane) {
                Some(agent) => Paragraph::new(agent.render_lines(&self.theme, inner_height)),
                None => Paragraph::new("starting agent…").style(self.theme.muted()),
            },
            PaneDesire::Search => match panes.search(pane) {
                Some(search) => Paragraph::new(search.render_lines(&self.theme)),
                None => {
                    Paragraph::new("search unavailable (no api handle)").style(self.theme.muted())
                }
            },
            PaneDesire::LifeOs => match panes.lifeos(pane) {
                Some(lifeos) => Paragraph::new(lifeos.render_lines(&self.theme)),
                None => {
                    Paragraph::new("life os unavailable (no api handle)").style(self.theme.muted())
                }
            },
            PaneDesire::Welcome => {
                let hints = workspace::welcome_lines();
                let pad = inner_height.saturating_sub(hints.len()) / 2;
                let mut lines: Vec<Line> = vec![Line::default(); pad];
                lines.extend(hints.into_iter().map(|(text, emphasized)| {
                    let style = if emphasized {
                        self.theme.title()
                    } else {
                        self.theme.muted()
                    };
                    Line::styled(text, style)
                }));
                Paragraph::new(lines).alignment(Alignment::Center)
            }
        };
        frame.render_widget(widget, content);

        // Right-edge column: marked scrollbar for editors (issue #29), a
        // dim seam between side-by-side panes otherwise.
        let strip = Rect {
            x: rect.x + rect.width - 1,
            y: content.y,
            width: 1,
            height: content.height,
        };
        if let PaneDesire::Editor(_) = &desire {
            if let Some(editor) = panes.editor(pane) {
                let (scroll, total) = editor.scroll_info();
                let marks = editor
                    .diagnostics
                    .iter()
                    .map(|(line, severity, _)| (*line, *severity))
                    .collect();
                frame.render_widget(
                    crate::decorations::MarkedScrollbar::new(
                        total,
                        scroll,
                        strip.height as usize,
                        marks,
                    ),
                    strip,
                );
            }
        } else if separator {
            let seam = vec![Line::from("│"); strip.height as usize];
            frame.render_widget(
                Paragraph::new(seam)
                    .style(Style::default().fg(theme::OUTLINE.resolve(self.theme.support))),
                strip,
            );
        }
    }

    /// The centered modal rectangle (palette / files). `pub(crate)` so the
    /// mouse router can hit-test clicks against the same geometry.
    pub(crate) fn modal_rect(&self, area: Rect) -> Rect {
        let width = (area.width * 6 / 10).clamp(20, 72).min(area.width);
        let height = 16.min(area.height);
        Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 3,
            width,
            height,
        }
    }

    fn draw_palette(&self, frame: &mut Frame, area: Rect) {
        let modal = self.modal_rect(area);
        let (style, set) = self.theme.border_emphasis();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(set)
            .border_style(style)
            .title(format!(" ▸ {} ", self.palette.query));
        let items: Vec<ListItem> = self
            .palette
            .matches()
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let style = if i == self.palette.selected {
                    self.theme.active_item()
                } else {
                    self.theme.text()
                };
                ListItem::new(c.title).style(style)
            })
            .collect();
        frame.render_widget(Clear, modal);
        frame.render_widget(List::new(items).block(block), modal);
    }

    fn draw_files_modal(&self, frame: &mut Frame, area: Rect) {
        let modal = self.modal_rect(area);
        let (style, set) = self.theme.border_emphasis();
        let (title, items, selected) = if let Some(picker) = &self.picker {
            let items: Vec<String> = picker.matches().into_iter().map(|(rel, _)| rel).collect();
            (format!(" ▸ {} ", picker.query), items, picker.selected)
        } else {
            return;
        };
        let rows: Vec<ListItem> = items
            .into_iter()
            .enumerate()
            .map(|(i, label)| {
                let style = if i == selected {
                    self.theme.active_item()
                } else {
                    self.theme.text()
                };
                ListItem::new(label).style(style)
            })
            .collect();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(set)
            .border_style(style)
            .title(title);
        frame.render_widget(Clear, modal);
        frame.render_widget(List::new(rows).block(block), modal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ColorSupport;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn shell() -> Shell {
        Shell::new(Theme::new(ColorSupport::TrueColor), "test-ws".into())
    }

    fn key(code: KeyCode, mods: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, mods))
    }

    #[test]
    fn keybindings_drive_split_focus_and_close() {
        let s = shell().on_event(&key(KeyCode::Char('s'), KeyModifiers::ALT));
        assert_eq!(s.layout.tab().root.panes().len(), 2);
        let s = s.on_event(&key(KeyCode::Char('n'), KeyModifiers::ALT));
        assert_eq!(s.layout.tab().focused, 0);
        let s = s.on_event(&key(KeyCode::Char('x'), KeyModifiers::ALT));
        assert_eq!(s.layout.tab().root.panes().len(), 1);
        assert!(s.running);
    }

    #[test]
    fn closing_the_last_pane_quits() {
        let s = shell().on_event(&key(KeyCode::Char('x'), KeyModifiers::ALT));
        assert!(!s.running);
    }

    #[test]
    fn palette_opens_captures_keys_and_invokes() {
        let s = shell().on_event(&key(KeyCode::Char('k'), KeyModifiers::CONTROL));
        assert!(s.palette.open);
        let s = s.on_event(&key(KeyCode::Char('q'), KeyModifiers::NONE));
        let s = s.on_event(&key(KeyCode::Char('u'), KeyModifiers::NONE));
        assert!(s.running);
        let s = s.on_event(&key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!s.palette.open);
        assert!(!s.running, "fuzzy 'qu' selects workbench: quit");
    }

    #[test]
    fn picker_opens_a_file_in_the_focused_pane_sharing_cwd() {
        let root = std::env::temp_dir().join(format!("wb_shell_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("pick_me.rs"), "fn a() {}\n").unwrap();
        let mut s = shell();
        s.status.cwd = root.display().to_string();

        let s = s.on_event(&key(KeyCode::Char('o'), KeyModifiers::CONTROL));
        assert!(s.picker.is_some(), "ctrl-o opens the picker at cwd");
        let s = "pickme".chars().fold(s, |s, c| {
            s.on_event(&key(KeyCode::Char(c), KeyModifiers::NONE))
        });
        let s = s.on_event(&key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(s.picker.is_none());
        let PaneDesire::Editor(path) = s.focused_desire() else {
            panic!("focused pane must become an editor");
        };
        assert!(path.ends_with("pick_me.rs"));

        // Alt-e flips the same pane back to its terminal.
        let s = s.on_event(&key(KeyCode::Char('e'), KeyModifiers::ALT));
        assert_eq!(s.focused_desire(), PaneDesire::Terminal);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn sidebar_toggles_and_owns_plain_keys_while_focused() {
        let s = shell();
        assert!(s.tree.is_some(), "sidebar starts open");
        let s = s.on_event(&key(KeyCode::Char('f'), KeyModifiers::ALT));
        assert!(s.tree.is_none(), "alt+f collapses it");
        let s = s.on_event(&key(KeyCode::Char('f'), KeyModifiers::ALT));
        assert!(s.tree.is_some());
        assert_eq!(s.chrome.focus, Region::Sidebar, "reopening focuses it");
        assert!(!s.forwards_to_pane(&key(KeyCode::Char('j'), KeyModifiers::NONE)));
        let s = s.on_event(&key(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(s.tree.as_ref().unwrap().selected, 1, "j moves selection");
        let s = s.on_event(&key(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(s.chrome.focus, Region::Center, "esc returns to center");
        assert!(s.tree.is_some(), "esc does not collapse the sidebar");
    }

    #[test]
    fn dock_toggles_and_takes_keyboard_focus() {
        let s = shell();
        assert!(s.chrome.dock_open, "terminal dock starts open");
        let s = s.on_event(&key(KeyCode::Char('j'), KeyModifiers::ALT));
        assert!(!s.chrome.dock_open);
        let s = s.on_event(&key(KeyCode::Char('j'), KeyModifiers::ALT));
        assert!(s.chrome.dock_open);
        assert_eq!(s.chrome.focus, Region::Dock);
        assert_eq!(s.focused_desire(), PaneDesire::Terminal);
        assert!(
            s.forwards_to_pane(&key(KeyCode::Char('l'), KeyModifiers::NONE)),
            "dock focus forwards typing to the dock terminal"
        );
        assert_eq!(s.effective_focused_pane(), DOCK_PANE);
    }

    #[test]
    fn center_starts_as_welcome_and_swallows_plain_keys() {
        let s = shell();
        assert_eq!(s.chrome.focus, Region::Center);
        assert_eq!(s.focused_desire(), PaneDesire::Welcome);
        assert!(!s.forwards_to_pane(&key(KeyCode::Char('j'), KeyModifiers::NONE)));
    }

    #[test]
    fn dock_shell_exit_closes_the_dock_and_reopening_rearms_it() {
        let s = shell();
        let s = s.on_event(&key(KeyCode::Char('j'), KeyModifiers::ALT)); // close
        let s = s.on_event(&key(KeyCode::Char('j'), KeyModifiers::ALT)); // open + focus
        let s = s.on_pane_exit(DOCK_PANE);
        assert!(!s.chrome.dock_open, "exit closes the dock");
        assert_eq!(s.chrome.focus, Region::Center);
        assert_eq!(s.desires.get(&DOCK_PANE), Some(&PaneDesire::Welcome));
        let s = s.on_event(&key(KeyCode::Char('j'), KeyModifiers::ALT));
        assert!(s.chrome.dock_open);
        assert_eq!(
            s.desires.get(&DOCK_PANE),
            Some(&PaneDesire::Terminal),
            "reopening the dock spawns a fresh shell"
        );
    }

    #[test]
    fn center_terminal_exit_closes_the_pane_or_reverts_to_welcome() {
        // Two panes: exiting one closes it and drops its desire.
        let s = shell().run_command(CommandId::SplitHorizontal);
        let s = s.run_command(CommandId::TerminalHere);
        let pane = s.layout.tab().focused;
        let s = s.on_pane_exit(pane);
        assert_eq!(s.layout.tab().root.panes().len(), 1);
        assert!(!s.desires.contains_key(&pane), "closed pane desire removed");
        // Last pane: exit reverts to welcome instead of quitting.
        let s = s.run_command(CommandId::TerminalHere);
        let pane = s.layout.tab().focused;
        let s = s.on_pane_exit(pane);
        assert!(s.running);
        assert_eq!(s.desires.get(&pane), Some(&PaneDesire::Welcome));
    }

    #[test]
    fn pane_rects_include_the_dock_even_while_hidden() {
        let s = shell();
        let area = Rect::new(0, 0, 120, 40);
        let rects = s.pane_rects(area);
        assert!(rects.iter().any(|(id, _)| *id == DOCK_PANE));
        let s = s.on_event(&key(KeyCode::Char('j'), KeyModifiers::ALT));
        assert!(!s.chrome.dock_open);
        let rects = s.pane_rects(area);
        assert!(
            rects.iter().any(|(id, _)| *id == DOCK_PANE),
            "hidden dock keeps its pane alive (shell session survives)"
        );
    }
}
