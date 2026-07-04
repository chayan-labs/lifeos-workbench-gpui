//! The Jobs/Pipelines pane: `GET /api/jobs` (the queue `lifeos-drain`
//! claims from) and `GET /api/pipeline/registry` (static DAG introspection -
//! "static, no tenant scoping" per its own doc comment, so this is a list of
//! declared stages, not a live-run visualizer).
//!
//! Honest gap: `lifeos-actions` is not linked into `lifeos-api` at all (only
//! `lifeos-drain` depends on it, per the route audit) - there is no endpoint
//! to poll for Actions status, so this pane says so plainly instead of
//! omitting the category or inventing a fake one.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, Task, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Sizable, StyledExt};
use serde_json::Value;

use super::api_host::{ApiHost, HostStatus};
use super::theme::pane_bg;

const POLL_MS: u64 = 150;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Jobs,
    Pipelines,
}

#[derive(Clone, Debug)]
struct JobRow {
    kind: String,
    status: String,
    created_at: i64,
}

#[derive(Clone, Debug)]
struct PipelineStage {
    name: String,
    agent: String,
    gated: bool,
}

#[derive(Clone, Debug)]
struct PipelineRow {
    id: String,
    stages: Vec<PipelineStage>,
}

#[derive(Default)]
struct State {
    busy: bool,
    error: Option<String>,
    jobs: Vec<JobRow>,
    pipelines: Vec<PipelineRow>,
    fetched_jobs: bool,
    fetched_pipelines: bool,
}

pub struct JobsView {
    api: ApiHost,
    tab: Tab,
    state: Arc<Mutex<State>>,
    focus: FocusHandle,
    _poll: Task<()>,
}

impl JobsView {
    pub fn new(api: ApiHost, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let view = Self {
            api,
            tab: Tab::Jobs,
            state: Arc::default(),
            focus: cx.focus_handle(),
            _poll: Task::ready(()),
        };
        view.fetch_jobs(cx);
        view
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
        let already_fetched = self
            .state
            .lock()
            .map(|s| match tab {
                Tab::Jobs => s.fetched_jobs,
                Tab::Pipelines => s.fetched_pipelines,
            })
            .unwrap_or(false);
        if !already_fetched {
            match tab {
                Tab::Jobs => self.fetch_jobs(cx),
                Tab::Pipelines => self.fetch_pipelines(cx),
            }
        }
        cx.notify();
    }

    fn fetch_jobs(&self, cx: &mut Context<Self>) {
        let Some((api, token)) = self.ready() else {
            cx.notify();
            return;
        };
        if let Ok(mut s) = self.state.lock() {
            s.busy = true;
        }
        let state = self.state.clone();
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api.get("/api/jobs", token.as_deref()).await;
            if let Ok(mut s) = state.lock() {
                s.busy = false;
                s.fetched_jobs = true;
                if response.is_success() {
                    s.jobs = response
                        .body
                        .as_array()
                        .map(|rows| {
                            rows.iter()
                                .map(|j| JobRow {
                                    kind: j["kind"].as_str().unwrap_or_default().to_string(),
                                    status: j["status"].as_str().unwrap_or_default().to_string(),
                                    created_at: j["created_at"].as_i64().unwrap_or_default(),
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

    fn fetch_pipelines(&self, cx: &mut Context<Self>) {
        let Some((api, token)) = self.ready() else {
            cx.notify();
            return;
        };
        if let Ok(mut s) = self.state.lock() {
            s.busy = true;
        }
        let state = self.state.clone();
        crate::ui::app::tokio_handle().spawn(async move {
            let response = api.get("/api/pipeline/registry", token.as_deref()).await;
            if let Ok(mut s) = state.lock() {
                s.busy = false;
                s.fetched_pipelines = true;
                if response.is_success() {
                    s.pipelines = parse_pipelines(&response.body);
                    s.error = None;
                } else {
                    s.error = Some(format!("error {}", response.status));
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
                    let busy = this.state.lock().map(|s| s.busy).unwrap_or(false);
                    cx.notify();
                    if busy {
                        this.start_poll(cx);
                    }
                });
            }
        })
        .detach();
    }
}

impl Focusable for JobsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for JobsView {
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
                Button::new("jobs-t-jobs")
                    .label("Jobs")
                    .small()
                    .when(tab == Tab::Jobs, |b| b.primary())
                    .when(tab != Tab::Jobs, |b| b.ghost())
                    .on_click(cx.listener(|this, _, _, cx| this.set_tab(Tab::Jobs, cx))),
            )
            .child(
                Button::new("jobs-t-pipelines")
                    .label("Pipelines")
                    .small()
                    .when(tab == Tab::Pipelines, |b| b.primary())
                    .when(tab != Tab::Pipelines, |b| b.ghost())
                    .on_click(cx.listener(|this, _, _, cx| this.set_tab(Tab::Pipelines, cx))),
            );

        let actions_note = div()
            .px_3()
            .py_1()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child("Actions status is not available: lifeos-actions is not linked into lifeos-api (only lifeos-drain), so no endpoint exists to poll it.");

        let s = self.state.lock();
        let body = match tab {
            Tab::Jobs => {
                let mut list = div()
                    .id("jobs-list")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_2()
                    .gap_1();
                match s.as_deref() {
                    Ok(s) if s.busy && !s.fetched_jobs => {
                        list = list.child(hint("loading jobs...", cx))
                    }
                    Ok(s) if s.error.is_some() => {
                        list = list.child(hint(s.error.as_deref().unwrap_or(""), cx))
                    }
                    Ok(s) if s.jobs.is_empty() => list = list.child(hint("no jobs queued", cx)),
                    Ok(s) => {
                        list = list.children(s.jobs.iter().map(|j| {
                            let color = match j.status.as_str() {
                                "done" => cx.theme().success,
                                "failed" => cx.theme().danger,
                                "running" => cx.theme().warning,
                                _ => cx.theme().muted_foreground,
                            };
                            div()
                                .h_flex()
                                .justify_between()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .child(
                                    div()
                                        .text_color(cx.theme().foreground)
                                        .child(j.kind.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(j.created_at.to_string()),
                                )
                                .child(div().text_xs().text_color(color).child(j.status.clone()))
                        }))
                    }
                    Err(_) => list = list.child(hint("state lock poisoned", cx)),
                }
                list
            }
            Tab::Pipelines => {
                let mut list = div()
                    .id("pipelines-list")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_2()
                    .gap_2();
                match s.as_deref() {
                    Ok(s) if s.busy && !s.fetched_pipelines => {
                        list = list.child(hint("loading registry...", cx))
                    }
                    Ok(s) if s.error.is_some() => {
                        list = list.child(hint(s.error.as_deref().unwrap_or(""), cx))
                    }
                    Ok(s) if s.pipelines.is_empty() => {
                        list = list.child(hint("no pipelines registered", cx))
                    }
                    Ok(s) => {
                        list = list.children(s.pipelines.iter().map(|p| {
                            div()
                                .v_flex()
                                .w_full()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(cx.theme().foreground)
                                        .child(p.id.clone()),
                                )
                                .children(p.stages.iter().map(|stage| {
                                    div()
                                        .pl_4()
                                        .h_flex()
                                        .justify_between()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{} \u{2192} {}", stage.name, stage.agent))
                                        .when(stage.gated, |d| d.child("gated"))
                                }))
                        }))
                    }
                    Err(_) => list = list.child(hint("state lock poisoned", cx)),
                }
                list
            }
        };
        drop(s);

        div()
            .track_focus(&self.focus)
            .key_context("Jobs")
            .v_flex()
            .size_full()
            .bg(pane_bg(cx))
            .child(tabs)
            .child(actions_note)
            .child(div().flex_1().min_h_0().child(body))
    }
}

fn hint(text: &str, cx: &Context<JobsView>) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}

fn parse_pipelines(body: &Value) -> Vec<PipelineRow> {
    body.as_array()
        .map(|rows| {
            rows.iter()
                .map(|p| PipelineRow {
                    id: p["id"].as_str().unwrap_or_default().to_string(),
                    stages: p["stages"]
                        .as_array()
                        .map(|stages| {
                            stages
                                .iter()
                                .map(|s| PipelineStage {
                                    name: s["name"].as_str().unwrap_or_default().to_string(),
                                    agent: s["agent"].as_str().unwrap_or_default().to_string(),
                                    gated: s["gated"].as_bool().unwrap_or(false),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_pipeline_registry_stages() {
        let body = json!([{
            "id": "ingest", "stages": [{"name": "fetch", "agent": "worker", "tool": null, "skill": null, "gate": null, "gated": false}]
        }]);
        let pipelines = parse_pipelines(&body);
        assert_eq!(pipelines.len(), 1);
        assert_eq!(pipelines[0].id, "ingest");
        assert_eq!(pipelines[0].stages[0].name, "fetch");
    }
}
