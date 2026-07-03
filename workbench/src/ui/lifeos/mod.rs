//! The Life OS pane: the structured face of the app.
//!
//! Browses the declarative module manifests (modules -> views) loaded straight
//! off disk, fetches each view's entities through the in-process `lifeos-api`
//! ([`ApiHost`]), and renders the manifest view-models ([`model`]) with native
//! gpui-component widgets: a table for `list`, side-by-side lanes for `board`, a
//! field stack + markdown for `detail`. Manifests are never forked; an unknown
//! `view.kind` degrades to the list renderer, exactly as the ratatui frontend
//! does.
//!
//! The module/view structure is real with or without a backend (it is disk
//! data). Entity rows come from the API when it is up; while it boots, or if the
//! bootstrap fails, the content region says so honestly instead of faking rows.

pub mod model;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Styled, Task, Window,
};
use gpui_component::{ActiveTheme, StyledExt};
use serde_json::Value;

use super::api_host::{ApiHost, HostStatus};
use crate::manifest::{self, ModuleManifest};
use model::{dispatch, BoardView, DetailView, ListView, Rendered};

const POLL_MS: u64 = 150;

/// Result of a fetch of one view's entities.
#[derive(Default)]
struct Fetched {
    entities: Vec<Value>,
    busy: bool,
    /// A tokio fetch is in flight (so the poll does not spawn a second).
    spawned: bool,
    error: Option<String>,
    /// The `(module_idx, view_idx)` this data belongs to, so a stale result
    /// never renders under a different view.
    key: (usize, usize),
}

pub struct LifeOsView {
    api: ApiHost,
    manifests: Vec<ModuleManifest>,
    load_errors: Vec<String>,
    module_idx: usize,
    view_idx: Option<usize>,
    fetched: Arc<Mutex<Fetched>>,
    focus: FocusHandle,
    content_scroll: ScrollHandle,
    _poll: Task<()>,
}

impl LifeOsView {
    pub fn new(api: ApiHost, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (manifests, load_errors) = match locate_modules_root() {
            Some(root) => manifest::load_all(&root),
            None => (
                Vec::new(),
                vec!["no modules directory found (set LIFEOS_MODULES_DIR)".into()],
            ),
        };
        Self {
            api,
            manifests,
            load_errors,
            module_idx: 0,
            view_idx: None,
            fetched: Arc::default(),
            focus: cx.focus_handle(),
            content_scroll: ScrollHandle::new(),
            _poll: Task::ready(()),
        }
    }

    pub fn handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn module(&self) -> Option<&ModuleManifest> {
        self.manifests.get(self.module_idx)
    }

    fn select_module(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.module_idx = idx;
        self.view_idx = None;
        if let Ok(mut f) = self.fetched.lock() {
            *f = Fetched::default();
        }
        cx.notify();
    }

    fn select_view(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.view_idx = Some(idx);
        self.request_view(cx);
    }

    /// Kick a fetch of the current view's entities, then poll for the result.
    fn request_view(&mut self, cx: &mut Context<Self>) {
        let key = (self.module_idx, self.view_idx.unwrap_or(0));
        if let Ok(mut f) = self.fetched.lock() {
            f.entities.clear();
            f.error = None;
            f.busy = true;
            f.spawned = false;
            f.key = key;
        }
        self.pump(cx);
        self.start_poll(cx);
    }

    /// Advance the fetch: spawn the request once the API is ready, or record an
    /// honest error if the bootstrap failed. A no-op once a request is in flight.
    fn pump(&mut self, _cx: &mut Context<Self>) {
        let (busy, spawned, key) = match self.fetched.lock() {
            Ok(f) => (f.busy, f.spawned, f.key),
            Err(_) => return,
        };
        if !busy || spawned {
            return;
        }
        match self.api.status() {
            HostStatus::Booting => {} // keep waiting; the poll retries
            HostStatus::Failed(e) => {
                if let Ok(mut f) = self.fetched.lock() {
                    f.busy = false;
                    f.error = Some(e);
                }
            }
            HostStatus::Ready(api, token) => {
                let Some(uri) = self.fetch_uri(key) else {
                    if let Ok(mut f) = self.fetched.lock() {
                        f.busy = false;
                    }
                    return;
                };
                if let Ok(mut f) = self.fetched.lock() {
                    f.spawned = true;
                }
                let fetched = self.fetched.clone();
                crate::ui::app::tokio_handle().spawn(async move {
                    let response = api.get(&uri, token.as_deref()).await;
                    if let Ok(mut f) = fetched.lock() {
                        if f.key != key {
                            return; // superseded by a newer selection
                        }
                        f.busy = false;
                        f.spawned = false;
                        if response.is_success() {
                            f.entities = response.body.as_array().cloned().unwrap_or_default();
                        } else {
                            f.error = Some(format!("api error {}", response.status));
                        }
                    }
                });
            }
        }
    }

    fn fetch_uri(&self, key: (usize, usize)) -> Option<String> {
        let module = self.manifests.get(key.0)?;
        let view = module.views.get(key.1)?;
        Some(format!(
            "/api/entity?module={}&type={}&limit=200",
            urlencode(&module.id),
            urlencode(&view.entity_type)
        ))
    }

    /// Repaint (and re-pump) every [`POLL_MS`] while a fetch is outstanding, then
    /// stop. Follows the terminal view's self-rescheduling tick.
    fn start_poll(&mut self, cx: &mut Context<Self>) {
        self._poll = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(POLL_MS))
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| {
                    this.pump(cx);
                    let busy = this.fetched.lock().map(|f| f.busy).unwrap_or(false);
                    cx.notify();
                    if busy {
                        this.start_poll(cx);
                    }
                });
            }
        });
    }

    // ---------------------------------------------------------------- render

    /// Left rail: the module list, then the selected module's views.
    fn rail(&self, cx: &Context<Self>) -> impl IntoElement {
        let module_idx = self.module_idx;
        let view_idx = self.view_idx;
        let mut rail = div()
            .v_flex()
            .w(gpui::px(220.0))
            .flex_shrink_0()
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .child(section_label("MODULES", cx));

        if self.manifests.is_empty() {
            rail = rail.child(muted_row("no modules loaded", cx));
            for err in &self.load_errors {
                rail = rail.child(muted_row(err, cx));
            }
            return rail;
        }

        rail = rail.child(
            div()
                .v_flex()
                .children(self.manifests.iter().enumerate().map(|(i, m)| {
                    rail_row(
                        ("os-mod", i),
                        &m.name,
                        &format!("{} views", m.views.len()),
                        i == module_idx,
                        cx,
                        cx.listener(move |this, _, _, cx| this.select_module(i, cx)),
                    )
                })),
        );

        if let Some(module) = self.module() {
            rail =
                rail.child(section_label("VIEWS", cx))
                    .child(div().v_flex().children(module.views.iter().enumerate().map(
                        |(i, v)| {
                            rail_row(
                                ("os-view", i),
                                &v.label,
                                &v.kind,
                                view_idx == Some(i),
                                cx,
                                cx.listener(move |this, _, _, cx| this.select_view(i, cx)),
                            )
                        },
                    )));
        }
        rail
    }

    /// The main content region: the selected view rendered, or a prompt/status.
    fn content(&self, cx: &Context<Self>) -> impl IntoElement {
        let base = div()
            .id("os-content")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.content_scroll)
            .p_4()
            .bg(cx.theme().background);

        let Some(view_idx) = self.view_idx else {
            return base.child(center_hint(
                "Select a module and a view",
                "The module list on the left is loaded from the same manifests the web app uses.",
                cx,
            ));
        };

        let Some(module) = self.module() else {
            return base.child(center_hint("No module", "", cx));
        };
        let Some(view) = module.views.get(view_idx) else {
            return base.child(center_hint("No view", "", cx));
        };
        let Some(entity_type) = module.entity_types.get(&view.entity_type) else {
            return base.child(center_hint(
                "Unknown entity type",
                &format!(
                    "view references '{}', not declared in the manifest",
                    view.entity_type
                ),
                cx,
            ));
        };

        let f = match self.fetched.lock() {
            Ok(f) => f,
            Err(_) => return base.child(center_hint("state lock poisoned", "", cx)),
        };
        if f.busy {
            let msg = match self.api.status() {
                HostStatus::Booting => "connecting to lifeos-api...",
                _ => "loading...",
            };
            return base.child(center_hint(msg, "", cx));
        }
        if let Some(err) = &f.error {
            return base.child(center_hint("Could not load entities", err, cx));
        }
        if f.entities.is_empty() {
            return base.child(center_hint(
                "No entries yet",
                &format!(
                    "this workspace has no {} records",
                    entity_type.label.to_lowercase()
                ),
                cx,
            ));
        }

        match dispatch(view, entity_type, &f.entities) {
            Rendered::List(v) => base.child(render_list(&v, cx)),
            Rendered::Board(b) => base.child(render_board(&b, cx)),
            Rendered::Detail(d) => base.child(render_detail(&d, cx)),
        }
    }
}

impl Focusable for LifeOsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for LifeOsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A first render kicks the initial fetch once a view is chosen; harmless
        // when idle (pump is a no-op unless a request is outstanding).
        div()
            .track_focus(&self.focus)
            .h_flex()
            .size_full()
            .child(self.rail(cx))
            .child(self.content(cx))
    }
}

// ------------------------------------------------------------- render helpers

fn render_list(v: &ListView, cx: &Context<LifeOsView>) -> impl IntoElement {
    let header = div()
        .h_flex()
        .w_full()
        .px_2()
        .py_1()
        .gap_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .text_xs()
        .font_semibold()
        .text_color(cx.theme().muted_foreground)
        .child(cell(&v.title_label, 3.0))
        .child(cell(&v.subtitle_label, 2.0))
        .child(cell(&v.badge_label, 1.0));

    let rows = div()
        .v_flex()
        .w_full()
        .children(v.rows.iter().enumerate().map(|(i, r)| {
            div()
                .h_flex()
                .w_full()
                .px_2()
                .py_1()
                .gap_2()
                .text_sm()
                .when(i % 2 == 1, |d| d.bg(cx.theme().secondary))
                .text_color(cx.theme().foreground)
                .child(cell(&r.title, 3.0))
                .child(
                    div()
                        .flex_basis(gpui::relative(2.0 / 6.0))
                        .min_w_0()
                        .text_color(cx.theme().muted_foreground)
                        .child(r.subtitle.clone()),
                )
                .child(badge(&r.badge, cx))
        }));

    div().v_flex().w_full().child(header).child(rows)
}

fn render_board(b: &BoardView, cx: &Context<LifeOsView>) -> impl IntoElement {
    div()
        .h_flex()
        .gap_3()
        .items_start()
        .children(b.columns.iter().map(|col| {
            div()
                .v_flex()
                .w(gpui::px(240.0))
                .flex_shrink_0()
                .gap_2()
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .font_semibold()
                                .text_sm()
                                .text_color(cx.theme().foreground)
                                .child(col.status.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{}", col.cards.len())),
                        ),
                )
                .children(col.cards.iter().map(|card| {
                    div()
                        .v_flex()
                        .gap_1()
                        .p_2()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().foreground)
                                .child(card.title.clone()),
                        )
                        .when(!card.badge.is_empty(), |d| d.child(badge(&card.badge, cx)))
                }))
        }))
}

fn render_detail(d: &DetailView, cx: &Context<LifeOsView>) -> impl IntoElement {
    let fields = div()
        .v_flex()
        .gap_1()
        .children(d.fields.iter().map(|(k, v)| {
            div()
                .h_flex()
                .gap_2()
                .text_sm()
                .child(
                    div()
                        .w(gpui::px(120.0))
                        .flex_shrink_0()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{k}:")),
                )
                .child(
                    div()
                        .min_w_0()
                        .text_color(cx.theme().foreground)
                        .child(v.clone()),
                )
        }));

    let mut root = div().v_flex().gap_3().w_full().child(fields);
    if !d.body.is_empty() {
        root = root
            .child(div().h(gpui::px(1.0)).w_full().bg(cx.theme().border))
            .child(render_markdown(&d.body, cx));
    }
    root
}

/// A deliberately small markdown renderer for detail bodies: headings, bullets,
/// and paragraphs. The visual tail (tables, images) is out of scope per the
/// cell-grid capability boundary; this covers what note bodies actually use.
fn render_markdown(src: &str, cx: &Context<LifeOsView>) -> impl IntoElement {
    let mut col = div().v_flex().gap_1().w_full();
    for line in src.lines() {
        let trimmed = line.trim_start();
        let el = if let Some(h) = trimmed.strip_prefix("# ") {
            div()
                .font_semibold()
                .text_lg()
                .text_color(cx.theme().foreground)
                .child(h.to_string())
        } else if let Some(h) = trimmed.strip_prefix("## ") {
            div()
                .font_semibold()
                .text_color(cx.theme().foreground)
                .child(h.to_string())
        } else if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            div()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(format!("\u{2022} {item}"))
        } else if trimmed.is_empty() {
            div().h(gpui::px(4.0))
        } else {
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(line.to_string())
        };
        col = col.child(el);
    }
    col
}

fn badge(text: &str, cx: &Context<LifeOsView>) -> gpui::Div {
    if text.is_empty() {
        return div().flex_basis(gpui::relative(1.0 / 6.0)).min_w_0();
    }
    div().flex_basis(gpui::relative(1.0 / 6.0)).min_w_0().child(
        div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .text_xs()
            .bg(cx.theme().accent)
            .text_color(cx.theme().accent_foreground)
            .child(text.to_string()),
    )
}

fn cell(text: &str, grow: f32) -> gpui::Div {
    div()
        .flex_basis(gpui::relative(grow / 6.0))
        .min_w_0()
        .child(text.to_string())
}

fn section_label(text: &'static str, cx: &Context<LifeOsView>) -> impl IntoElement {
    div()
        .px_3()
        .pt_3()
        .pb_1()
        .text_xs()
        .font_semibold()
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

fn muted_row(text: &str, cx: &Context<LifeOsView>) -> impl IntoElement {
    div()
        .px_3()
        .py_1()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}

fn rail_row(
    id: (&'static str, usize),
    title: &str,
    meta: &str,
    active: bool,
    cx: &Context<LifeOsView>,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .h_flex()
        .items_center()
        .justify_between()
        .w_full()
        .px_3()
        .py_1()
        .cursor_pointer()
        .text_sm()
        .when(active, |d| d.bg(cx.theme().accent))
        .text_color(if active {
            cx.theme().accent_foreground
        } else {
            cx.theme().sidebar_foreground
        })
        .child(div().min_w_0().child(title.to_string()))
        .child(
            div()
                .text_xs()
                .flex_shrink_0()
                .text_color(cx.theme().muted_foreground)
                .child(meta.to_string()),
        )
        .on_mouse_down(MouseButton::Left, on_click)
}

fn center_hint(title: &str, sub: &str, cx: &Context<LifeOsView>) -> impl IntoElement {
    div()
        .v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .child(
            div()
                .font_semibold()
                .text_color(cx.theme().foreground)
                .child(title.to_string()),
        )
        .when(!sub.is_empty(), |d| {
            d.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(sub.to_string()),
            )
        })
}

/// Find the modules directory: explicit env override, the working directory's
/// checkout, or the repo relative to this crate (dev builds).
pub fn locate_modules_root() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
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

/// Percent-encode a query fragment (shared shape with the recall pane).
pub fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_string()
            } else {
                c.to_string().bytes().map(|b| format!("%{b:02X}")).collect()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modules_root_is_locatable_in_the_repo_checkout() {
        let root = locate_modules_root().expect("dev checkout has modules/");
        assert!(root.join("tasks/module.js").is_file());
    }

    #[test]
    fn urlencode_escapes_non_alphanumerics() {
        assert_eq!(urlencode("a b/c"), "a%20b%2Fc");
    }
}
