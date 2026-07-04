//! The agent pane: an ACP client surface.
//!
//! Wraps the renderer-agnostic [`AcpAgent`] (the ACP client - we spawn the
//! configured agent and converse, we do NOT write a model) in a gpui view: a
//! header showing/changing the resolved agent command, a scrolling
//! transcript, a focusable input line, and the inline diff-review UI for
//! staged edits. Agent-proposed `fs/write_text_file`s are never applied
//! silently; they arrive as [`crate::acp::ProposedEdit`]s the user accepts or
//! rejects per hunk (mouse or keyboard), and only then does the ACP response
//! resolve. When the configured agent binary is absent the pane says so honestly
//! rather than pretending an agent is attached.

pub mod detect;

use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Task, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Sizable, StyledExt};

use crate::acp::{AcpAgent, Entry};
use crate::ui::config::{self, Config};
use crate::ui::theme::pane_bg;

const POLL_MS: u64 = 120;

pub struct AgentView {
    agent: Option<AcpAgent>,
    /// The resolved command this pane's agent was spawned with. A running
    /// agent process is not hot-swapped when this changes - the next spawn
    /// (pane recreation) picks up a newly-picked command.
    agent_cmd: String,
    input: String,
    /// Selected hunk index when reviewing (for keyboard navigation).
    review_cursor: usize,
    reviewing: bool,
    /// Whether the "Change..." agent picker is expanded.
    picker_open: bool,
    picker_custom: String,
    focus: FocusHandle,
    _poll: Task<()>,
}

/// Resolve the agent command in explicit precedence order: `config.lua`'s
/// `agent.command` > `WORKBENCH_AGENT_CMD` env > first auto-detected `$PATH`
/// match > the hardcoded Claude Code ACP adapter fallback.
pub fn resolve_agent_command(config: &Config) -> String {
    if let Some(cmd) = &config.agent.command {
        return cmd.clone();
    }
    if let Ok(cmd) = std::env::var("WORKBENCH_AGENT_CMD") {
        return cmd;
    }
    if let Some(found) = detect::scan_path().into_iter().next() {
        return found.name;
    }
    "claude-code-acp".to_string()
}

/// The toolbelt passed at session/new: this same binary re-entered as a stdio
/// MCP server over the in-process lifeos-api.
pub fn toolbelt_servers() -> Vec<serde_json::Value> {
    let Ok(exe) = std::env::current_exe() else {
        return Vec::new();
    };
    vec![serde_json::json!({
        "name": "lifeos",
        "command": exe.display().to_string(),
        "args": ["--mcp"],
        "env": []
    })]
}

impl AgentView {
    pub fn new(config: &Config, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let agent_cmd = resolve_agent_command(config);
        let agent = AcpAgent::spawn(&agent_cmd, &cwd, toolbelt_servers());
        let mut view = Self {
            agent,
            agent_cmd,
            input: String::new(),
            review_cursor: 0,
            reviewing: false,
            picker_open: false,
            picker_custom: String::new(),
            focus: cx.focus_handle(),
            _poll: Task::ready(()),
        };
        // A running agent streams on a background thread; keep repainting.
        view.start_poll(cx);
        view
    }

    pub fn handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn toggle_picker(&mut self, cx: &mut Context<Self>) {
        self.picker_open = !self.picker_open;
        cx.notify();
    }

    /// Persist a newly-picked agent command to `config.lua` and update the
    /// displayed command. Does not respawn the running agent process.
    fn choose_agent(&mut self, command: String, cx: &mut Context<Self>) {
        if let Err(e) = config::write_back("agent", "command", &command) {
            self.agent_cmd = format!("{command} (could not save: {e})");
        } else {
            self.agent_cmd = command;
        }
        self.picker_open = false;
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        if let Some(agent) = &self.agent {
            agent.prompt(&text);
        }
        self.input.clear();
        self.start_poll(cx);
        cx.notify();
    }

    fn on_key(&mut self, e: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.picker_open {
            return self.on_picker_key(e, cx);
        }
        let ks = &e.keystroke;
        match ks.key.as_str() {
            "enter" => self.submit(cx),
            "backspace" => {
                self.input.pop();
                cx.notify();
            }
            _ => {
                if !ks.modifiers.platform && !ks.modifiers.control {
                    if let Some(ch) = &ks.key_char {
                        if !ch.is_empty() && !ch.chars().any(|c| c.is_control()) {
                            self.input.push_str(ch);
                            cx.notify();
                        }
                    }
                }
            }
        }
    }

    /// Keys while the agent picker is expanded: type a custom path, Enter to
    /// use it. Routed through the same root `on_key` (not a second
    /// `track_focus`) since only one focus handle exists on this pane.
    fn on_picker_key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        let ks = &e.keystroke;
        match ks.key.as_str() {
            "enter" => {
                let path = self.picker_custom.trim().to_string();
                if !path.is_empty() {
                    self.choose_agent(path, cx);
                }
            }
            "escape" => {
                self.picker_open = false;
                cx.notify();
            }
            "backspace" => {
                self.picker_custom.pop();
                cx.notify();
            }
            _ => {
                if !ks.modifiers.platform && !ks.modifiers.control {
                    if let Some(ch) = &ks.key_char {
                        if !ch.is_empty() && !ch.chars().any(|c| c.is_control()) {
                            self.picker_custom.push_str(ch);
                            cx.notify();
                        }
                    }
                }
            }
        }
    }

    fn toggle_hunk(&mut self, hunk: usize, cx: &mut Context<Self>) {
        if let Some(agent) = &self.agent {
            agent.toggle_hunk(0, hunk);
        }
        cx.notify();
    }

    fn resolve(&mut self, accept: bool, cx: &mut Context<Self>) {
        if let Some(agent) = &self.agent {
            agent.resolve_edit(0, accept);
        }
        self.reviewing = false;
        self.review_cursor = 0;
        self.start_poll(cx);
        cx.notify();
    }

    fn start_poll(&mut self, cx: &mut Context<Self>) {
        self._poll = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(POLL_MS))
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| {
                    let busy = this
                        .agent
                        .as_ref()
                        .and_then(|a| a.conversation.lock().ok().map(|c| c.busy))
                        .unwrap_or(false);
                    cx.notify();
                    // Keep polling while the agent is thinking; one extra tick
                    // after it settles flushes the final entries.
                    if busy {
                        this.start_poll(cx);
                    }
                });
            }
        });
    }

    // ---------------------------------------------------------------- render

    fn transcript(&self, cx: &Context<Self>) -> impl IntoElement {
        let mut col = div()
            .id("agent-transcript")
            .v_flex()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_3()
            .gap_1();

        let Some(agent) = &self.agent else {
            return col
                .child(
                    div()
                        .font_semibold()
                        .text_color(cx.theme().danger)
                        .child("agent unavailable"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "could not start `{}` - pick a detected agent above, or set \
                             WORKBENCH_AGENT_CMD to your ACP adapter",
                            self.agent_cmd
                        )),
                );
        };
        let Ok(c) = agent.conversation.lock() else {
            return col;
        };
        for entry in &c.entries {
            let el = match entry {
                Entry::User(t) => div()
                    .font_semibold()
                    .text_color(cx.theme().foreground)
                    .child(format!("\u{25B8} {t}")),
                Entry::Agent(t) => div().text_color(cx.theme().foreground).child(t.clone()),
                Entry::ToolCall(t) => div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("\u{2699} {t}")),
                Entry::Info(t) => div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("\u{00B7} {t}")),
            };
            col = col.child(el);
        }
        if c.busy {
            col = col.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("\u{2026} thinking"),
            );
        }
        col
    }

    fn review_block(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let agent = self.agent.as_ref()?;
        let c = agent.conversation.lock().ok()?;
        let edit = c.edits.first()?;
        let path = edit.path.display().to_string();
        let cursor = self.review_cursor;

        let mut block = div()
            .v_flex()
            .gap_1()
            .p_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(cx.theme().foreground)
                            .child(format!("review {path}")),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_1()
                            .child(
                                Button::new("edit-accept")
                                    .label("Accept")
                                    .primary()
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| this.resolve(true, cx))),
                            )
                            .child(
                                Button::new("edit-reject")
                                    .label("Reject")
                                    .danger()
                                    .small()
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.resolve(false, cx)),
                                    ),
                            ),
                    ),
            );

        for (i, hunk) in edit.hunks.iter().enumerate() {
            let on = edit.accepted.contains(&i);
            let marker = if on { "[x]" } else { "[ ]" };
            let selected = self.reviewing && i == cursor;
            let mut h = div()
                .id(("hunk", i))
                .v_flex()
                .gap_0p5()
                .p_1()
                .rounded_md()
                .cursor_pointer()
                .when(selected, |d| d.bg(cx.theme().accent))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.reviewing = true;
                        this.review_cursor = i;
                        this.toggle_hunk(i, cx);
                    }),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .child(format!(
                            "{marker} hunk {} @ line {}",
                            i + 1,
                            hunk.old_start + 1
                        )),
                );
            for old in &hunk.old {
                h = h.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(format!("- {old}")),
                );
            }
            for new in &hunk.new {
                h = h.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().success)
                        .child(format!("+ {new}")),
                );
            }
            block = block.child(h);
        }
        Some(block)
    }

    /// Header row: the resolved agent command + a "Change..." toggle.
    fn header(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .items_center()
            .justify_between()
            .w_full()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("agent"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(cx.theme().foreground)
                            .child(self.agent_cmd.clone()),
                    ),
            )
            .child(
                Button::new("agent-change")
                    .label(if self.picker_open {
                        "Close"
                    } else {
                        "Change..."
                    })
                    .ghost()
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_picker(cx))),
            )
    }

    /// The agent picker: detected `$PATH` candidates + a custom-path field.
    /// Selecting one persists to `config.lua` (see `choose_agent`); a running
    /// agent process isn't hot-swapped, so this only affects the next spawn.
    fn picker(&self, cx: &Context<Self>) -> impl IntoElement {
        let found = detect::scan_path();
        let mut block = div()
            .v_flex()
            .gap_1()
            .p_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary);

        if found.is_empty() {
            block = block.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("no known agent CLIs found on $PATH"),
            );
        }
        for (i, a) in found.into_iter().enumerate() {
            let name = a.name.clone();
            let path_str = a.path.display().to_string();
            block = block.child(
                div()
                    .id(("agent-candidate", i))
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(a.name.clone())
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(path_str),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| this.choose_agent(name.clone(), cx)),
                    ),
            );
        }

        block.child(
            div()
                .h_flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("custom path (type, then Enter):"),
                )
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .child(format!("{}\u{2588}", self.picker_custom)),
                ),
        )
    }

    fn input_line(&self, cx: &Context<Self>) -> impl IntoElement {
        let enabled = self.agent.is_some();
        div()
            .h_flex()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .gap_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("\u{25B8}"),
            )
            .child(
                div()
                    .flex_1()
                    .text_color(if enabled {
                        cx.theme().foreground
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(if enabled {
                        format!("{}\u{2588}", self.input)
                    } else {
                        "agent offline".to_string()
                    }),
            )
    }
}

impl Focusable for AgentView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for AgentView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .track_focus(&self.focus)
            .key_context("Agent")
            .on_key_down(cx.listener(Self::on_key))
            .v_flex()
            .size_full()
            .bg(pane_bg(cx))
            .child(self.header(cx));
        if self.picker_open {
            root = root.child(self.picker(cx));
        }
        root = root.child(self.transcript(cx));
        if let Some(block) = self.review_block(cx) {
            root = root.child(block);
        }
        root.child(self.input_line(cx))
    }
}
