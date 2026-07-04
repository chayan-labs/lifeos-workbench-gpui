//! The Self-extension pane: `POST /api/module-request` + `GET
//! /api/module-request/:id`. The cloud side only enqueues a `module_build`
//! job - the Mac harness elsewhere in this system drains it and does the
//! actual scaffold build - so this pane submits + polls status honestly
//! rather than faking a build step it cannot perform itself.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, StatefulInteractiveElement, Styled, Task, Window,
};
use gpui_component::button::Button;
use gpui_component::{ActiveTheme, Sizable, StyledExt};
use serde_json::json;

use super::api_host::{ApiHost, HostStatus};
use super::theme::pane_bg;

const POLL_MS: u64 = 400;

#[derive(Clone, Debug)]
struct ModuleRequest {
    id: String,
    prompt: String,
    status: String,
    error: Option<String>,
}

#[derive(Default)]
struct State {
    busy: bool,
    error: Option<String>,
    requests: Vec<ModuleRequest>,
}

pub struct SelfExtendView {
    api: ApiHost,
    prompt: String,
    state: Arc<Mutex<State>>,
    focus: FocusHandle,
    _poll: Task<()>,
}

impl SelfExtendView {
    pub fn new(api: ApiHost, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            api,
            prompt: String::new(),
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

    fn on_key(&mut self, e: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let ks = &e.keystroke;
        match ks.key.as_str() {
            "enter" => self.submit(cx),
            "backspace" => {
                self.prompt.pop();
                cx.notify();
            }
            _ => {
                if !ks.modifiers.platform && !ks.modifiers.control {
                    if let Some(ch) = &ks.key_char {
                        if !ch.is_empty() && !ch.chars().any(|c| c.is_control()) {
                            self.prompt.push_str(ch);
                            cx.notify();
                        }
                    }
                }
            }
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let prompt = self.prompt.trim().to_string();
        if prompt.is_empty() {
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
        self.prompt.clear();
        let state = self.state.clone();
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api
                .post(
                    "/api/module-request",
                    json!({ "prompt": prompt.clone() }),
                    token.as_deref(),
                )
                .await;
            if let Ok(mut s) = state.lock() {
                s.busy = false;
                if response.is_success() {
                    let id = response.body["id"].as_str().unwrap_or_default().to_string();
                    s.requests.insert(
                        0,
                        ModuleRequest {
                            id,
                            prompt,
                            status: response.body["status"]
                                .as_str()
                                .unwrap_or("queued")
                                .to_string(),
                            error: None,
                        },
                    );
                } else {
                    s.error = Some(format!("error {}", response.status));
                }
            }
        });
        self.start_poll(cx);
    }

    fn poll_requests(&self, cx: &mut Context<Self>) {
        let Some((api, token)) = self.ready() else {
            return;
        };
        let ids: Vec<String> = self
            .state
            .lock()
            .map(|s| {
                s.requests
                    .iter()
                    .filter(|r| !matches!(r.status.as_str(), "installed" | "failed"))
                    .map(|r| r.id.clone())
                    .collect()
            })
            .unwrap_or_default();
        if ids.is_empty() {
            return;
        }
        let state = self.state.clone();
        crate::ui::app::tokio_handle().spawn(async move {
            for id in ids {
                let response = api
                    .get(&format!("/api/module-request/{id}"), token.as_deref())
                    .await;
                if response.is_success() {
                    if let Ok(mut s) = state.lock() {
                        if let Some(r) = s.requests.iter_mut().find(|r| r.id == id) {
                            r.status = response.body["status"]
                                .as_str()
                                .unwrap_or(&r.status)
                                .to_string();
                            r.error = response.body["error"].as_str().map(String::from);
                        }
                    }
                }
            }
        });
        self.start_poll(cx);
    }

    fn start_poll(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(POLL_MS))
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| {
                    this.poll_requests(cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

impl Focusable for SelfExtendView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SelfExtendView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child("describe a module \u{203A}"),
            )
            .child(
                div()
                    .flex_1()
                    .text_color(cx.theme().foreground)
                    .child(format!("{}\u{2588}", self.prompt)),
            )
            .child(
                Button::new("selfextend-submit")
                    .label("Submit")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            );

        let s = self.state.lock();
        let mut list = div()
            .id("selfextend-list")
            .v_flex()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_2()
            .gap_1();
        match s.as_deref() {
            Ok(s) if s.error.is_some() => {
                list = list.child(hint(s.error.as_deref().unwrap_or(""), cx))
            }
            Ok(s) if s.requests.is_empty() => {
                list = list.child(hint(
                    "no module requests yet - describe one above and press Enter",
                    cx,
                ))
            }
            Ok(s) => {
                list = list.children(s.requests.iter().map(|r| {
                    let color = match r.status.as_str() {
                        "installed" => cx.theme().success,
                        "failed" => cx.theme().danger,
                        "building" => cx.theme().warning,
                        _ => cx.theme().muted_foreground,
                    };
                    div()
                        .v_flex()
                        .w_full()
                        .px_2()
                        .py_1()
                        .gap_0p5()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .child(
                            div()
                                .h_flex()
                                .justify_between()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().foreground)
                                        .child(r.prompt.clone()),
                                )
                                .child(div().text_xs().text_color(color).child(r.status.clone())),
                        )
                        .when_some(r.error.clone(), |d, e| {
                            d.child(div().text_xs().text_color(cx.theme().danger).child(e))
                        })
                }))
            }
            Err(_) => list = list.child(hint("state lock poisoned", cx)),
        }
        drop(s);

        div()
            .track_focus(&self.focus)
            .key_context("SelfExtend")
            .on_key_down(cx.listener(Self::on_key))
            .v_flex()
            .size_full()
            .bg(pane_bg(cx))
            .child(input_line)
            .child(list)
    }
}

fn hint(text: &str, cx: &Context<SelfExtendView>) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}
