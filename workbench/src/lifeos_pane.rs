//! Life OS pane: the structured face of the app. Browses the declarative
//! module manifests (modules → views), fetches entities through the
//! in-process `lifeos-api` handle, and renders each view via the manifest
//! renderer set (`views.rs`) - list/table, board, detail. No manifest is
//! ever forked; unknown kinds degrade per DESIGN.md.

use crate::api::InProcessApi;
use crate::manifest::{self, ModuleManifest};
use crate::search_pane::urlencode;
use crate::theme::Theme;
use crate::views::{self, Rendered};
use crossterm::event::KeyCode;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Find the modules directory: explicit env override, the working
/// directory's checkout, or the repo relative to this crate (dev builds).
pub fn locate_modules_root() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("LIFEOS_MODULES_DIR") {
        candidates.push(dir.into());
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("modules"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../modules"));
    candidates.into_iter().find(|p| p.is_dir())
}

/// Where the browser currently is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Level {
    Modules,
    Views,
    Open,
}

#[derive(Default)]
struct Fetched {
    entities: Vec<Value>,
    busy: bool,
    error: Option<String>,
}

pub struct LifeOsPane {
    api: InProcessApi,
    token: Option<String>,
    manifests: Vec<ModuleManifest>,
    load_errors: Vec<String>,
    level: Level,
    module_idx: usize,
    view_idx: usize,
    /// Cursor within the current list (modules, views, or entity rows).
    pub selected: usize,
    fetched: Arc<Mutex<Fetched>>,
}

/// Lines the breadcrumb + spacer occupy before the first selectable row.
const HEADER_ROWS: usize = 2;

impl LifeOsPane {
    pub fn new(api: InProcessApi, token: Option<String>) -> Self {
        let (manifests, load_errors) = match locate_modules_root() {
            Some(root) => manifest::load_all(&root),
            None => (
                Vec::new(),
                vec!["no modules directory found (set LIFEOS_MODULES_DIR)".into()],
            ),
        };
        Self {
            api,
            token,
            manifests,
            load_errors,
            level: Level::Modules,
            module_idx: 0,
            view_idx: 0,
            selected: 0,
            fetched: Arc::default(),
        }
    }

    fn module(&self) -> Option<&ModuleManifest> {
        self.manifests.get(self.module_idx)
    }

    fn row_count(&self) -> usize {
        match self.level {
            Level::Modules => self.manifests.len(),
            Level::Views => self.module().map(|m| m.views.len()).unwrap_or(0),
            Level::Open => self.fetched.lock().map(|f| f.entities.len()).unwrap_or(0),
        }
    }

    /// Fetch the open view's entities through the in-process API.
    fn fetch(&mut self) {
        let Some(module) = self.module() else {
            return;
        };
        let Some(view) = module.views.get(self.view_idx) else {
            return;
        };
        let uri = format!(
            "/api/entity?module={}&type={}&limit=200",
            urlencode(&module.id),
            urlencode(&view.entity_type)
        );
        if let Ok(mut f) = self.fetched.lock() {
            f.busy = true;
            f.error = None;
            f.entities.clear();
        }
        let (api, token, fetched) = (self.api.clone(), self.token.clone(), self.fetched.clone());
        tokio::spawn(async move {
            let response = api.get(&uri, token.as_deref()).await;
            if let Ok(mut f) = fetched.lock() {
                f.busy = false;
                if response.is_success() {
                    f.entities = response.body.as_array().cloned().unwrap_or_default();
                } else {
                    f.error = Some(format!("api error {}", response.status));
                }
            }
        });
    }

    fn descend(&mut self) {
        match self.level {
            Level::Modules if self.selected < self.manifests.len() => {
                self.module_idx = self.selected;
                self.level = Level::Views;
                self.selected = 0;
            }
            Level::Views if self.selected < self.row_count() => {
                self.view_idx = self.selected;
                self.level = Level::Open;
                self.selected = 0;
                self.fetch();
            }
            _ => {}
        }
    }

    fn ascend(&mut self) {
        match self.level {
            Level::Open => {
                self.level = Level::Views;
                self.selected = self.view_idx;
            }
            Level::Views => {
                self.level = Level::Modules;
                self.selected = self.module_idx;
            }
            Level::Modules => {}
        }
    }

    /// Feed one key; returns true when the pane consumed it.
    pub fn on_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                let n = self.row_count();
                if n > 0 {
                    self.selected = (self.selected + 1).min(n - 1);
                }
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.descend(),
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => self.ascend(),
            _ => return false,
        }
        true
    }

    /// Mouse click on a content row: select it and activate (single-click
    /// open, matching the sidebar). `row` is relative to the content top.
    pub fn on_click(&mut self, row: usize) {
        if self.level == Level::Open {
            return;
        }
        let Some(idx) = row.checked_sub(HEADER_ROWS) else {
            return;
        };
        if idx < self.row_count() {
            self.selected = idx;
            self.descend();
        }
    }

    fn breadcrumb(&self, theme: &Theme) -> Line<'static> {
        let mut spans = vec![Span::styled(" life os".to_string(), theme.title())];
        if self.level != Level::Modules {
            if let Some(m) = self.module() {
                spans.push(Span::styled(" ▸ ".to_string(), theme.muted()));
                spans.push(Span::styled(m.name.clone(), theme.text()));
            }
        }
        if self.level == Level::Open {
            if let Some(v) = self.module().and_then(|m| m.views.get(self.view_idx)) {
                spans.push(Span::styled(" ▸ ".to_string(), theme.muted()));
                spans.push(Span::styled(v.label.clone(), theme.text()));
            }
        }
        Line::from(spans)
    }

    fn list_lines(&self, rows: Vec<(String, String)>, theme: &Theme) -> Vec<Line<'static>> {
        rows.into_iter()
            .enumerate()
            .map(|(i, (title, meta))| {
                let style = if i == self.selected {
                    theme.active_item()
                } else {
                    theme.text()
                };
                Line::from(vec![
                    Span::styled(format!(" {title:<32} "), style),
                    Span::styled(meta, theme.muted()),
                ])
            })
            .collect()
    }

    fn open_view_lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        let Some(module) = self.module() else {
            return Vec::new();
        };
        let Some(view) = module.views.get(self.view_idx) else {
            return Vec::new();
        };
        let Ok(f) = self.fetched.lock() else {
            return Vec::new();
        };
        if f.busy {
            return vec![Line::styled("loading…".to_string(), theme.muted())];
        }
        if let Some(err) = &f.error {
            return vec![Line::styled(err.clone(), theme.muted())];
        }
        let Some(entity_type) = module.entity_types.get(&view.entity_type) else {
            return vec![Line::styled(
                format!("unknown entity type '{}'", view.entity_type),
                theme.muted(),
            )];
        };
        if f.entities.is_empty() {
            return vec![Line::styled("no entries yet".to_string(), theme.muted())];
        }
        match views::dispatch(view, entity_type, &f.entities) {
            Rendered::List(mut v) => {
                v.cursor = self.selected.min(v.rows.len().saturating_sub(1));
                v.lines(&entity_type.label, theme)
            }
            Rendered::Board(b) => {
                let mut lines = Vec::new();
                for column in &b.columns {
                    lines.push(Line::styled(
                        format!("── {} ({})", column.status, column.cards.len()),
                        Style::default()
                            .fg(crate::theme::PRIMARY.resolve(theme.support))
                            .add_modifier(Modifier::BOLD),
                    ));
                    for card in &column.cards {
                        lines.push(Line::from(vec![
                            Span::styled(format!("   {} ", card.title), theme.text()),
                            Span::styled(card.badge.clone(), theme.muted()),
                        ]));
                    }
                }
                lines
            }
            Rendered::Detail(d) => d.lines(theme),
        }
    }

    pub fn render_lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines = vec![self.breadcrumb(theme), Line::default()];
        match self.level {
            Level::Modules => {
                if self.manifests.is_empty() {
                    lines.push(Line::styled(
                        " no modules loaded".to_string(),
                        theme.muted(),
                    ));
                    for err in &self.load_errors {
                        lines.push(Line::styled(format!(" {err}"), theme.muted()));
                    }
                } else {
                    let rows = self
                        .manifests
                        .iter()
                        .map(|m| (m.name.clone(), format!("{} views", m.views.len())))
                        .collect();
                    lines.extend(self.list_lines(rows, theme));
                }
            }
            Level::Views => {
                let rows = self
                    .module()
                    .map(|m| {
                        m.views
                            .iter()
                            .map(|v| (v.label.clone(), v.kind.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                lines.extend(self.list_lines(rows, theme));
            }
            Level::Open => lines.extend(self.open_view_lines(theme)),
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modules_root_is_locatable_in_the_repo_checkout() {
        let root = locate_modules_root().expect("dev checkout has modules/");
        assert!(root.join("tasks/module.js").is_file());
    }
}
