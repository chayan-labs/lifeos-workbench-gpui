//! The Recall pane: hybrid search over the in-process `lifeos-api`.
//!
//! The query goes to `/api/search` (FTS5 lexical + memvec semantic, RRF-fused,
//! lexical-only degradation) through the same [`ApiHost`] the Life OS pane uses -
//! no socket, no second process. The pane is a focusable query line plus a
//! results list; typing edits the query (captured like the command palette),
//! Enter fires the search on the tokio runtime, and a poll tick repaints when the
//! hits land.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Task, Window,
};
use gpui_component::{ActiveTheme, StyledExt};
use serde_json::Value;

use super::api_host::{ApiHost, HostStatus};
use super::lifeos::urlencode;
use super::theme::pane_bg;

const POLL_MS: u64 = 150;

/// One fused hit, reduced to what the pane renders.
#[derive(Clone, Debug)]
pub struct Hit {
    pub title: String,
    pub module: String,
    pub entity_type: String,
    pub id: String,
}

#[derive(Default)]
struct Results {
    hits: Vec<Hit>,
    mode: String,
    busy: bool,
    error: Option<String>,
}

pub struct RecallView {
    api: ApiHost,
    query: String,
    selected: usize,
    results: Arc<Mutex<Results>>,
    focus: FocusHandle,
    _poll: Task<()>,
}

impl RecallView {
    pub fn new(api: ApiHost, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            api,
            query: String::new(),
            selected: 0,
            results: Arc::default(),
            focus: cx.focus_handle(),
            _poll: Task::ready(()),
        }
    }

    pub fn handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn on_key(&mut self, e: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let ks = &e.keystroke;
        match ks.key.as_str() {
            "enter" => self.run(cx),
            "backspace" => {
                self.query.pop();
                cx.notify();
            }
            "up" => {
                self.selected = self.selected.saturating_sub(1);
                cx.notify();
            }
            "down" => {
                let n = self.results.lock().map(|r| r.hits.len()).unwrap_or(0);
                if n > 0 {
                    self.selected = (self.selected + 1).min(n - 1);
                }
                cx.notify();
            }
            _ => {
                if !ks.modifiers.platform && !ks.modifiers.control {
                    if let Some(ch) = &ks.key_char {
                        if !ch.is_empty() && !ch.chars().any(|c| c.is_control()) {
                            self.query.push_str(ch);
                            cx.notify();
                        }
                    }
                }
            }
        }
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            return;
        }
        self.selected = 0;
        let (api, token) = match self.api.status() {
            HostStatus::Ready(api, token) => (api, token),
            HostStatus::Booting => {
                if let Ok(mut r) = self.results.lock() {
                    r.busy = true;
                    r.error = Some("connecting to lifeos-api...".into());
                }
                return;
            }
            HostStatus::Failed(e) => {
                if let Ok(mut r) = self.results.lock() {
                    r.busy = false;
                    r.error = Some(e);
                }
                cx.notify();
                return;
            }
        };
        if let Ok(mut r) = self.results.lock() {
            r.busy = true;
            r.error = None;
        }
        let results = self.results.clone();
        let uri = format!("/api/search?q={}&limit=20", urlencode(&query));
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api.get(&uri, token.as_deref()).await;
            if let Ok(mut r) = results.lock() {
                r.busy = false;
                if response.is_success() {
                    let (hits, mode) = parse_hits(&response.body);
                    r.hits = hits;
                    r.mode = mode;
                    r.error = None;
                } else {
                    r.error = Some(format!("error {}", response.status));
                }
            }
        });
        self.start_poll(cx);
    }

    fn start_poll(&mut self, cx: &mut Context<Self>) {
        self._poll = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(POLL_MS))
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| {
                    let busy = this.results.lock().map(|r| r.busy).unwrap_or(false);
                    cx.notify();
                    if busy {
                        this.start_poll(cx);
                    }
                });
            }
        });
    }
}

impl Focusable for RecallView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for RecallView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query_line = div()
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
                    .child("recall \u{203A}"),
            )
            .child(
                div()
                    .flex_1()
                    .text_color(cx.theme().foreground)
                    .child(format!("{}\u{2588}", self.query)),
            );

        let r = self.results.lock();
        let mut list = div()
            .id("recall-list")
            .v_flex()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_2()
            .gap_0p5();

        match r.as_deref() {
            Ok(r) if r.busy => {
                list = list.child(hint_row("searching...", cx));
            }
            Ok(r) if r.error.is_some() => {
                list = list.child(hint_row(r.error.as_deref().unwrap_or(""), cx));
            }
            Ok(r) if r.hits.is_empty() && !r.mode.is_empty() => {
                list = list.child(hint_row(&format!("{} \u{00B7} 0 hits", r.mode), cx));
            }
            Ok(r) if r.hits.is_empty() => {
                list = list.child(hint_row("type a query and press Enter", cx));
            }
            Ok(r) => {
                let selected = self.selected;
                list = list
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} \u{00B7} {} hits", r.mode, r.hits.len())),
                    )
                    .children(r.hits.iter().enumerate().map(|(i, hit)| {
                        let is_sel = i == selected;
                        div()
                            .id(("hit", i))
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
                            .child(div().min_w_0().child(hit.title.clone()))
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("{} \u{00B7} {}", hit.module, hit.entity_type)),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.selected = i;
                                    cx.notify();
                                }),
                            )
                    }));
            }
            Err(_) => {
                list = list.child(hint_row("results lock poisoned", cx));
            }
        }
        drop(r);

        div()
            .track_focus(&self.focus)
            .key_context("Recall")
            .on_key_down(cx.listener(Self::on_key))
            .v_flex()
            .size_full()
            .bg(pane_bg(cx))
            .child(query_line)
            .child(list)
    }
}

fn hint_row(text: &str, cx: &Context<RecallView>) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}

fn parse_hits(body: &Value) -> (Vec<Hit>, String) {
    let mode = body["mode"].as_str().unwrap_or("?").to_string();
    let hits = body["results"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    Some(Hit {
                        title: r["title"].as_str().unwrap_or("(untitled)").to_string(),
                        module: r["module"].as_str().unwrap_or_default().to_string(),
                        entity_type: r["type"].as_str().unwrap_or_default().to_string(),
                        id: r["id"].as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    (hits, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hits_and_mode_from_the_api_shape() {
        let body = serde_json::json!({
            "query": "q", "mode": "lexical",
            "results": [{"id": "e1", "title": "Ship it", "module": "tasks", "type": "task"}]
        });
        let (hits, mode) = parse_hits(&body);
        assert_eq!(mode, "lexical");
        assert_eq!(hits[0].title, "Ship it");
        assert_eq!(hits[0].module, "tasks");
    }
}
