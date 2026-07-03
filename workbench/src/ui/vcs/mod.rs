//! The VCS pane: a client over `/api/vcs/*` (`lifeos-vcs`).
//!
//! `lifeos-vcs` versions Life OS *entities* (a `files`-module entity with a
//! `blob_ref`), not arbitrary filesystem paths - there is no route that
//! takes a path, only `entity_id` - so this pane's input is an entity id,
//! not a reuse of `ui/file_tree.rs`'s filesystem tree (that tree walks the
//! Workbench's own working directory, a different namespace entirely; wiring
//! it to VCS would silently conflate the two, which this project's own
//! "don't fake it" ethos rules out).
//!
//! `GET /api/vcs/diff` already returns line-tagged content
//! (`{tag: equal|insert|delete, text}`), a different shape from
//! `crate::diff::Hunk` (which powers the agent pane's *local* text diff) -
//! so this renders that shape directly rather than forcing it through
//! `diff.rs`, which would need a lossy reconstruction of two full texts
//! first.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Task, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Sizable, StyledExt};
use serde_json::json;

use super::api_host::{ApiHost, HostStatus};
use super::theme::pane_bg;

const POLL_MS: u64 = 150;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    History,
    Refs,
}

#[derive(Clone, Debug)]
struct VersionEntry {
    blob_ref: String,
    author: String,
    message: String,
    ts: i64,
}

#[derive(Clone, Debug)]
struct DiffLine {
    tag: String,
    text: String,
}

#[derive(Clone, Debug)]
struct RefEntry {
    name: String,
    snapshot_ref: String,
}

#[derive(Default)]
struct State {
    busy: bool,
    error: Option<String>,
    history: Vec<VersionEntry>,
    selected: Vec<usize>,
    diff_summary: Option<String>,
    diff_supported: bool,
    diff_lines: Vec<DiffLine>,
    branches: Vec<RefEntry>,
    tags: Vec<RefEntry>,
    ref_action_result: Option<String>,
}

pub struct VcsView {
    api: ApiHost,
    tab: Tab,
    entity_id: String,
    ref_name: String,
    state: Arc<Mutex<State>>,
    focus: FocusHandle,
    _poll: Task<()>,
}

impl VcsView {
    pub fn new(api: ApiHost, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            api,
            tab: Tab::History,
            entity_id: String::new(),
            ref_name: String::new(),
            state: Arc::default(),
            focus: cx.focus_handle(),
            _poll: Task::ready(()),
        }
    }

    pub fn handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn ready(&self) -> Option<(crate::api::InProcessApi, Option<String>)> {
        match self.api.status() {
            HostStatus::Ready(api, token) => Some((api, token)),
            HostStatus::Booting => {
                if let Ok(mut s) = self.state.lock() {
                    s.error = Some("connecting to lifeos-api...".into());
                }
                None
            }
            HostStatus::Failed(e) => {
                if let Ok(mut s) = self.state.lock() {
                    s.error = Some(e);
                }
                None
            }
        }
    }

    fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        if tab == Tab::Refs {
            self.fetch_refs(cx);
        }
        cx.notify();
    }

    fn on_key(&mut self, e: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let ks = &e.keystroke;
        let field = match self.tab {
            Tab::History => &mut self.entity_id,
            Tab::Refs => &mut self.ref_name,
        };
        match ks.key.as_str() {
            "enter" => match self.tab {
                Tab::History => self.fetch_history(cx),
                Tab::Refs => {}
            },
            "backspace" => {
                field.pop();
                cx.notify();
            }
            _ => {
                if !ks.modifiers.platform && !ks.modifiers.control {
                    if let Some(ch) = &ks.key_char {
                        if !ch.is_empty() && !ch.chars().any(|c| c.is_control()) {
                            field.push_str(ch);
                            cx.notify();
                        }
                    }
                }
            }
        }
    }

    fn fetch_history(&mut self, cx: &mut Context<Self>) {
        let entity_id = self.entity_id.trim().to_string();
        if entity_id.is_empty() {
            return;
        }
        let Some((api, token)) = self.ready() else {
            cx.notify();
            return;
        };
        if let Ok(mut s) = self.state.lock() {
            s.busy = true;
            s.error = None;
            s.selected.clear();
            s.diff_summary = None;
            s.diff_lines.clear();
        }
        let state = self.state.clone();
        let uri = format!(
            "/api/vcs/history?entity_id={}",
            super::lifeos::urlencode(&entity_id)
        );
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api.get(&uri, token.as_deref()).await;
            if let Ok(mut s) = state.lock() {
                s.busy = false;
                if response.is_success() {
                    s.history = response
                        .body
                        .as_array()
                        .map(|rows| {
                            rows.iter()
                                .map(|e| VersionEntry {
                                    blob_ref: e["blob_ref"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
                                    author: e["author"].as_str().unwrap_or_default().to_string(),
                                    message: e["message"].as_str().unwrap_or_default().to_string(),
                                    ts: e["ts"].as_i64().unwrap_or_default(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    s.error = None;
                } else {
                    s.error = Some(format!("error {}", response.status));
                }
            }
        });
        self.start_poll(cx);
    }

    fn toggle_select(&mut self, idx: usize, cx: &mut Context<Self>) {
        let entity_id = self.entity_id.trim().to_string();
        let mut run_diff = None;
        if let Ok(mut s) = self.state.lock() {
            if s.selected.contains(&idx) {
                s.selected.retain(|i| *i != idx);
            } else {
                s.selected.push(idx);
                if s.selected.len() > 2 {
                    s.selected.remove(0);
                }
            }
            if s.selected.len() == 2 {
                let mut ordered = s.selected.clone();
                ordered.sort();
                let old = s.history.get(ordered[0]).map(|h| h.blob_ref.clone());
                let new = s.history.get(ordered[1]).map(|h| h.blob_ref.clone());
                if let (Some(old), Some(new)) = (old, new) {
                    run_diff = Some((old, new));
                }
            }
        }
        cx.notify();
        if let Some((old, new)) = run_diff {
            self.fetch_diff(entity_id, old, new, cx);
        }
    }

    fn fetch_diff(&mut self, entity_id: String, old: String, new: String, cx: &mut Context<Self>) {
        let Some((api, token)) = self.ready() else {
            cx.notify();
            return;
        };
        if let Ok(mut s) = self.state.lock() {
            s.busy = true;
        }
        let state = self.state.clone();
        let uri = format!(
            "/api/vcs/diff?entity_id={}&old={}&new={}",
            super::lifeos::urlencode(&entity_id),
            super::lifeos::urlencode(&old),
            super::lifeos::urlencode(&new)
        );
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api.get(&uri, token.as_deref()).await;
            if let Ok(mut s) = state.lock() {
                s.busy = false;
                if response.is_success() {
                    let body = &response.body;
                    s.diff_supported = body["supported"].as_bool().unwrap_or(false);
                    s.diff_summary = body["summary"].as_str().map(String::from).or_else(|| {
                        body["blocked_by"]
                            .as_str()
                            .map(|b| format!("unsupported: {b}"))
                    });
                    s.diff_lines = body["lines"]
                        .as_array()
                        .map(|rows| {
                            rows.iter()
                                .map(|l| DiffLine {
                                    tag: l["tag"].as_str().unwrap_or("equal").to_string(),
                                    text: l["text"].as_str().unwrap_or_default().to_string(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    s.error = None;
                } else {
                    s.error = Some(format!("error {}", response.status));
                }
            }
        });
        self.start_poll(cx);
    }

    fn fetch_refs(&mut self, cx: &mut Context<Self>) {
        let Some((api, token)) = self.ready() else {
            cx.notify();
            return;
        };
        if let Ok(mut s) = self.state.lock() {
            s.busy = true;
        }
        let state = self.state.clone();
        crate::ui::app::tokio_handle().spawn(async move {
            for (kind, is_branch) in [("branch", true), ("tag", false)] {
                let response = api
                    .get(&format!("/api/vcs/refs?kind={kind}"), token.as_deref())
                    .await;
                if let Ok(mut s) = state.lock() {
                    if response.is_success() {
                        let entries: Vec<RefEntry> = response
                            .body
                            .as_array()
                            .map(|rows| {
                                rows.iter()
                                    .map(|r| RefEntry {
                                        name: r["name"].as_str().unwrap_or_default().to_string(),
                                        snapshot_ref: r["snapshot_ref"]
                                            .as_str()
                                            .unwrap_or_default()
                                            .to_string(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        if is_branch {
                            s.branches = entries;
                        } else {
                            s.tags = entries;
                        }
                    } else {
                        s.error = Some(format!("error {}", response.status));
                    }
                }
            }
            if let Ok(mut s) = state.lock() {
                s.busy = false;
            }
        });
        self.start_poll(cx);
    }

    fn create_ref(&mut self, kind: &'static str, cx: &mut Context<Self>) {
        let name = self.ref_name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let Some((api, token)) = self.ready() else {
            cx.notify();
            return;
        };
        if let Ok(mut s) = self.state.lock() {
            s.busy = true;
        }
        let state = self.state.clone();
        let uri = format!("/api/vcs/{kind}");
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api
                .post(&uri, json!({ "name": name }), token.as_deref())
                .await;
            if let Ok(mut s) = state.lock() {
                s.busy = false;
                s.ref_action_result = Some(if response.is_success() {
                    format!("{kind} created")
                } else {
                    format!("error {}: {}", response.status, response.body)
                });
            }
        });
        self.start_poll(cx);
        self.fetch_refs(cx);
    }

    fn start_poll(&mut self, cx: &mut Context<Self>) {
        self._poll = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(POLL_MS))
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| {
                    let busy = this.state.lock().map(|s| s.busy).unwrap_or(false);
                    cx.notify();
                    if busy {
                        this.start_poll(cx);
                    }
                });
            }
        });
    }
}

impl Focusable for VcsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for VcsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.tab;
        let tabs = div()
            .h_flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                tab_button("vcs-t-history", "History", tab == Tab::History)
                    .on_click(cx.listener(|this, _, _, cx| this.set_tab(Tab::History, cx))),
            )
            .child(
                tab_button("vcs-t-refs", "Refs", tab == Tab::Refs)
                    .on_click(cx.listener(|this, _, _, cx| this.set_tab(Tab::Refs, cx))),
            );

        let s = self.state.lock();
        let body = match tab {
            Tab::History => {
                let input_line = div()
                    .h_flex()
                    .items_center()
                    .w_full()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("entity_id \u{203A}"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(cx.theme().foreground)
                            .child(format!("{}\u{2588}", self.entity_id)),
                    );
                let mut list = div()
                    .id("vcs-history")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_2()
                    .gap_1();
                match s.as_deref() {
                    Ok(s) if s.busy && s.history.is_empty() => {
                        list = list.child(hint("loading history...", cx))
                    }
                    Ok(s) if s.error.is_some() => {
                        list = list.child(hint(s.error.as_deref().unwrap_or(""), cx))
                    }
                    Ok(s) if s.history.is_empty() => {
                        list = list.child(hint("type an entity id and press Enter", cx))
                    }
                    Ok(s) => {
                        let selected = s.selected.clone();
                        list = list.children(s.history.iter().enumerate().map(|(i, v)| {
                            let is_sel = selected.contains(&i);
                            div()
                                .id(("version", i))
                                .h_flex()
                                .items_center()
                                .justify_between()
                                .w_full()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .cursor_pointer()
                                .text_sm()
                                .when(is_sel, |d| d.bg(cx.theme().accent))
                                .text_color(if is_sel {
                                    cx.theme().accent_foreground
                                } else {
                                    cx.theme().foreground
                                })
                                .child(div().min_w_0().child(format!(
                                    "{} \u{00B7} {}",
                                    &v.blob_ref[..v.blob_ref.len().min(10)],
                                    v.message
                                )))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{} \u{00B7} {}", v.author, v.ts)),
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| this.toggle_select(i, cx)),
                                )
                        }));
                        if let Some(summary) = &s.diff_summary {
                            list = list.child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("diff: {summary}")),
                            );
                            if s.diff_supported {
                                list = list.children(s.diff_lines.iter().map(|l| {
                                    let (bg, prefix) = match l.tag.as_str() {
                                        "insert" => (cx.theme().success.opacity(0.15), "+"),
                                        "delete" => (cx.theme().danger.opacity(0.15), "-"),
                                        _ => (gpui::transparent_black(), " "),
                                    };
                                    div()
                                        .px_2()
                                        .bg(bg)
                                        .text_xs()
                                        .font_family("monospace")
                                        .text_color(cx.theme().foreground)
                                        .child(format!("{prefix} {}", l.text))
                                }));
                            }
                        }
                    }
                    Err(_) => list = list.child(hint("state lock poisoned", cx)),
                }
                div().v_flex().size_full().child(input_line).child(list)
            }
            Tab::Refs => {
                let input_line = div()
                    .h_flex()
                    .items_center()
                    .w_full()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("name \u{203A}"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(cx.theme().foreground)
                            .child(format!("{}\u{2588}", self.ref_name)),
                    )
                    .child(
                        Button::new("vcs-new-branch")
                            .label("New branch")
                            .small()
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| this.create_ref("branch", cx))),
                    )
                    .child(
                        Button::new("vcs-new-tag")
                            .label("New tag")
                            .small()
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| this.create_ref("tag", cx))),
                    );
                let mut list = div()
                    .id("vcs-refs")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_2()
                    .gap_2();
                match s.as_deref() {
                    Ok(s) => {
                        if let Some(msg) = &s.ref_action_result {
                            list = list.child(hint(msg, cx));
                        }
                        list = list
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("BRANCHES"),
                            )
                            .children(s.branches.iter().map(ref_row))
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("TAGS"),
                            )
                            .children(s.tags.iter().map(ref_row));
                    }
                    Err(_) => list = list.child(hint("state lock poisoned", cx)),
                }
                div().v_flex().size_full().child(input_line).child(list)
            }
        };
        drop(s);

        div()
            .track_focus(&self.focus)
            .key_context("Vcs")
            .on_key_down(cx.listener(Self::on_key))
            .v_flex()
            .size_full()
            .bg(pane_bg(cx))
            .child(tabs)
            .child(div().flex_1().min_h_0().child(body))
    }
}

fn tab_button(id: &'static str, label: &'static str, active: bool) -> Button {
    let b = Button::new(id).label(label).small();
    if active {
        b.primary()
    } else {
        b.ghost()
    }
}

fn hint(text: &str, cx: &Context<VcsView>) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}

fn ref_row(r: &RefEntry) -> impl IntoElement {
    div()
        .h_flex()
        .justify_between()
        .px_2()
        .py_0p5()
        .text_sm()
        .child(r.name.clone())
        .child(
            div()
                .text_xs()
                .child(r.snapshot_ref[..r.snapshot_ref.len().min(10)].to_string()),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn history_response_shape_parses() {
        let body = json!([{"blob_ref": "b1", "parent_blob_ref": null, "author": "cli", "message": "init", "ts": 1}]);
        let rows = body
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|e| VersionEntry {
                        blob_ref: e["blob_ref"].as_str().unwrap_or_default().to_string(),
                        author: e["author"].as_str().unwrap_or_default().to_string(),
                        message: e["message"].as_str().unwrap_or_default().to_string(),
                        ts: e["ts"].as_i64().unwrap_or_default(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].blob_ref, "b1");
    }

    #[test]
    fn diff_lines_parse_tags_and_text() {
        let body = json!({
            "supported": true, "kind": "text", "summary": "1 line added",
            "inserted": 1, "deleted": 0,
            "lines": [{"tag": "equal", "text": "a"}, {"tag": "insert", "text": "b"}],
        });
        let lines: Vec<DiffLine> = body["lines"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|l| DiffLine {
                        tag: l["tag"].as_str().unwrap_or("equal").to_string(),
                        text: l["text"].as_str().unwrap_or_default().to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].tag, "insert");
    }
}
