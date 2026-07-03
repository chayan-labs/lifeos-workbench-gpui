//! The Memory pane: a thin client over the one shared `lifeos-memory`
//! service (`/api/memory/*`), reached through the same in-process
//! [`ApiHost`] every other pane uses.
//!
//! This must stay a thin client (no local cache, no client-side
//! consolidation) so memory stays identical no matter which LifeOS surface -
//! Telegram, web, or this desktop app - reads or writes it; see the
//! `lifeos-shared-memory-invariant` project memory. The pane is three tabs
//! over that one surface: hybrid Recall (with activation scores, the actual
//! differentiator from the generic Recall pane's `/api/search`), read-only
//! Rules, and a read-only Inspect ledger, plus a Consolidate-now action.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, StatefulInteractiveElement, Styled, Task, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Sizable, StyledExt};
use serde_json::{json, Value};

use super::api_host::{ApiHost, HostStatus};
use super::theme::pane_bg;

const POLL_MS: u64 = 150;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Recall,
    Rules,
    Inspect,
}

#[derive(Clone, Debug)]
struct RecallHit {
    content: String,
    activation: f64,
}

#[derive(Clone, Debug)]
struct InspectEntry {
    event_type: String,
    ts: i64,
}

#[derive(Default)]
struct State {
    busy: bool,
    error: Option<String>,
    outcome: String,
    hits: Vec<RecallHit>,
    rules: Vec<String>,
    inspect: Vec<InspectEntry>,
    inspect_stats: Option<Value>,
    consolidate_result: Option<String>,
}

pub struct MemoryView {
    api: ApiHost,
    tab: Tab,
    query: String,
    state: Arc<Mutex<State>>,
    focus: FocusHandle,
    _poll: Task<()>,
}

impl MemoryView {
    pub fn new(api: ApiHost, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            api,
            tab: Tab::Recall,
            query: String::new(),
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
        match tab {
            Tab::Rules => self.fetch_rules(cx),
            Tab::Inspect => self.fetch_inspect(cx),
            Tab::Recall => {}
        }
        cx.notify();
    }

    fn on_key(&mut self, e: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.tab != Tab::Recall {
            return;
        }
        let ks = &e.keystroke;
        match ks.key.as_str() {
            "enter" => self.run_recall(cx),
            "backspace" => {
                self.query.pop();
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

    fn run_recall(&mut self, cx: &mut Context<Self>) {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            return;
        }
        let Some((api, token)) = self.ready() else {
            cx.notify();
            return;
        };
        if let Ok(mut s) = self.state.lock() {
            s.busy = true;
            s.error = None;
        }
        let state = self.state.clone();
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api
                .post(
                    "/api/memory/recall",
                    json!({ "query": query }),
                    token.as_deref(),
                )
                .await;
            if let Ok(mut s) = state.lock() {
                s.busy = false;
                if response.is_success() {
                    parse_recall_into(&response.body, &mut s);
                } else {
                    s.error = Some(format!("error {}", response.status));
                }
            }
        });
        self.start_poll(cx);
    }

    fn fetch_rules(&mut self, cx: &mut Context<Self>) {
        let Some((api, token)) = self.ready() else {
            cx.notify();
            return;
        };
        if let Ok(mut s) = self.state.lock() {
            s.busy = true;
        }
        let state = self.state.clone();
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api.get("/api/memory/rules", token.as_deref()).await;
            if let Ok(mut s) = state.lock() {
                s.busy = false;
                if response.is_success() {
                    s.rules = response.body["rules"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(String::from))
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

    fn fetch_inspect(&mut self, cx: &mut Context<Self>) {
        let Some((api, token)) = self.ready() else {
            cx.notify();
            return;
        };
        if let Ok(mut s) = self.state.lock() {
            s.busy = true;
        }
        let state = self.state.clone();
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api.get("/api/memory/inspect", token.as_deref()).await;
            if let Ok(mut s) = state.lock() {
                s.busy = false;
                if response.is_success() {
                    s.inspect = response.body["entries"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|e| InspectEntry {
                                    event_type: e["type"].as_str().unwrap_or_default().to_string(),
                                    ts: e["ts"].as_i64().unwrap_or_default(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    s.inspect_stats = response.body.get("stats").cloned();
                    s.error = None;
                } else {
                    s.error = Some(format!("error {}", response.status));
                }
            }
        });
        self.start_poll(cx);
    }

    fn consolidate_now(&mut self, cx: &mut Context<Self>) {
        let Some((api, token)) = self.ready() else {
            cx.notify();
            return;
        };
        if let Ok(mut s) = self.state.lock() {
            s.busy = true;
            s.consolidate_result = None;
        }
        let state = self.state.clone();
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api
                .post("/api/memory/sleep", json!({}), token.as_deref())
                .await;
            if let Ok(mut s) = state.lock() {
                s.busy = false;
                s.consolidate_result = Some(if response.is_success() {
                    format!("consolidated: {}", response.body)
                } else {
                    format!("error {}", response.status)
                });
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

impl Focusable for MemoryView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for MemoryView {
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
                tab_button("t-recall", "Recall", tab == Tab::Recall)
                    .on_click(cx.listener(|this, _, _, cx| this.set_tab(Tab::Recall, cx))),
            )
            .child(
                tab_button("t-rules", "Rules", tab == Tab::Rules)
                    .on_click(cx.listener(|this, _, _, cx| this.set_tab(Tab::Rules, cx))),
            )
            .child(
                tab_button("t-inspect", "Inspect", tab == Tab::Inspect)
                    .on_click(cx.listener(|this, _, _, cx| this.set_tab(Tab::Inspect, cx))),
            )
            .child(div().flex_1())
            .child(
                Button::new("consolidate")
                    .label("Consolidate now")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| this.consolidate_now(cx))),
            );

        let s = self.state.lock();
        let body = match tab {
            Tab::Recall => {
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
                let mut list = div()
                    .id("mem-recall-list")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_2()
                    .gap_1();
                match s.as_deref() {
                    Ok(s) if s.busy => list = list.child(hint("recalling...", cx)),
                    Ok(s) if s.error.is_some() => {
                        list = list.child(hint(s.error.as_deref().unwrap_or(""), cx))
                    }
                    Ok(s) if !s.outcome.is_empty() => {
                        list = list
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "outcome: {} \u{00B7} {} hits",
                                        s.outcome,
                                        s.hits.len()
                                    )),
                            )
                            .children(s.hits.iter().map(|h| {
                                div()
                                    .v_flex()
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(cx.theme().secondary)
                                    .child(
                                        div()
                                            .h_flex()
                                            .justify_between()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("activation {:.3}", h.activation)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .child(h.content.clone()),
                                    )
                            }))
                    }
                    _ => list = list.child(hint("type a query and press Enter", cx)),
                }
                div().v_flex().size_full().child(query_line).child(list)
            }
            Tab::Rules => {
                let mut list = div()
                    .id("mem-rules-list")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_2()
                    .gap_1();
                match s.as_deref() {
                    Ok(s) if s.busy => list = list.child(hint("loading rules...", cx)),
                    Ok(s) if s.error.is_some() => {
                        list = list.child(hint(s.error.as_deref().unwrap_or(""), cx))
                    }
                    Ok(s) if s.rules.is_empty() => list = list.child(hint("no active rules", cx)),
                    Ok(s) => {
                        list = list.children(s.rules.iter().map(|r| {
                            div()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .text_color(cx.theme().foreground)
                                .child(r.clone())
                        }))
                    }
                    Err(_) => list = list.child(hint("state lock poisoned", cx)),
                }
                div().v_flex().size_full().child(list)
            }
            Tab::Inspect => {
                let mut list = div()
                    .id("mem-inspect-list")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_2()
                    .gap_1();
                match s.as_deref() {
                    Ok(s) if s.busy => list = list.child(hint("loading ledger...", cx)),
                    Ok(s) if s.error.is_some() => {
                        list = list.child(hint(s.error.as_deref().unwrap_or(""), cx))
                    }
                    Ok(s) if s.inspect.is_empty() => {
                        list = list.child(hint("no ledger events yet", cx))
                    }
                    Ok(s) => {
                        if let Some(stats) = &s.inspect_stats {
                            list = list.child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(stats.to_string()),
                            );
                        }
                        list = list.children(s.inspect.iter().map(|e| {
                            div()
                                .h_flex()
                                .justify_between()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .text_color(cx.theme().foreground)
                                .child(e.event_type.clone())
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(e.ts.to_string()),
                                )
                        }))
                    }
                    Err(_) => list = list.child(hint("state lock poisoned", cx)),
                }
                div().v_flex().size_full().child(list)
            }
        };
        let consolidate_hint = s
            .as_deref()
            .ok()
            .and_then(|s| s.consolidate_result.clone())
            .map(|msg| {
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(msg)
            });
        drop(s);

        div()
            .track_focus(&self.focus)
            .key_context("Memory")
            .on_key_down(cx.listener(Self::on_key))
            .v_flex()
            .size_full()
            .bg(pane_bg(cx))
            .child(tabs)
            .when_some(consolidate_hint, |d, h| d.child(h))
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

fn hint(text: &str, cx: &Context<MemoryView>) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}

/// Parses `POST /api/memory/recall`'s `RecallOutcome` (externally tagged as
/// `{"outcome": "recalled"|"skipped"|"abstained", ...}`) into the pane's
/// flat hit list.
fn parse_recall_into(body: &Value, s: &mut State) {
    let outcome = body["outcome"].as_str().unwrap_or("?").to_string();
    s.outcome = outcome.clone();
    s.hits = if outcome == "recalled" {
        body["memories"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|m| RecallHit {
                        content: m["content"].as_str().unwrap_or_default().to_string(),
                        activation: m["breakdown"]["activation"].as_f64().unwrap_or(0.0),
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
}

// The mode buttons in `mode_button` (workspace_view.rs) use `Button` too, so
// this file's button helper mirrors that shape rather than inventing a new one.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_recalled_outcome_with_activation_scores() {
        let body = json!({
            "outcome": "recalled",
            "expanded_graph": false,
            "memories": [{"content": "likes dark roast", "breakdown": {"activation": 0.87}}],
        });
        let mut s = State::default();
        parse_recall_into(&body, &mut s);
        assert_eq!(s.outcome, "recalled");
        assert_eq!(s.hits.len(), 1);
        assert!((s.hits[0].activation - 0.87).abs() < 1e-9);
    }

    #[test]
    fn parses_a_skipped_outcome_with_no_hits() {
        let body = json!({ "outcome": "skipped", "reason": "small talk" });
        let mut s = State::default();
        parse_recall_into(&body, &mut s);
        assert_eq!(s.outcome, "skipped");
        assert!(s.hits.is_empty());
    }

    #[test]
    fn parses_an_abstained_outcome_with_no_hits() {
        let body = json!({ "outcome": "abstained", "top_activation": 0.1, "threshold": 0.3 });
        let mut s = State::default();
        parse_recall_into(&body, &mut s);
        assert_eq!(s.outcome, "abstained");
        assert!(s.hits.is_empty());
    }
}
