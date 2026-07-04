//! The Settings pane: a full setting guide.
//!
//! Every field in [`Config`] gets one entry in [`catalog::SETTINGS_CATALOG`]
//! with a plain-English description, so this pane is documentation and a live
//! editor at once - not just a reference page. Changing a control mutates an
//! in-memory copy of `Config`, persists it via [`config::write_back`] (a
//! minimal textual upsert into `config.lua`, preserving everything else the
//! user has in that file), and, for hot-appliable settings (theme mode,
//! fonts), re-applies immediately on the live window/theme. Settings that only
//! take effect the next time their pane (re)opens say so honestly rather than
//! pretending to be live.

pub mod catalog;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
    WindowBackgroundAppearance,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Sizable, StyledExt};

use super::config::{self, Config};
use super::theme::{chrome_bg, glass_edge, pane_bg, GlassMode};
use catalog::{SettingKind, SETTINGS_CATALOG};

pub struct SettingsView {
    config: Config,
    section_idx: usize,
    /// The entry currently being text-edited (Text/Number kinds), if any.
    editing: Option<(usize, usize)>,
    edit_buffer: String,
    hint: String,
    focus: FocusHandle,
}

/// One past the real catalog sections: a read-only "Integrations" pointer to
/// the Memory/Trading/VCS/Jobs/Self-extension panes' connectivity, so the
/// settings guide really is the map of everything the app can do.
fn integrations_idx() -> usize {
    SETTINGS_CATALOG.len()
}

impl SettingsView {
    pub fn new(config: &Config, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            config: config.clone(),
            section_idx: 0,
            editing: None,
            edit_buffer: String::new(),
            hint: String::new(),
            focus: cx.focus_handle(),
        }
    }

    pub fn handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn select_section(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.section_idx = idx;
        self.editing = None;
        cx.notify();
    }

    fn start_edit(&mut self, section_idx: usize, entry_idx: usize, cx: &mut Context<Self>) {
        let entry = &SETTINGS_CATALOG[section_idx].entries[entry_idx];
        self.edit_buffer = (entry.get)(&self.config);
        self.editing = Some((section_idx, entry_idx));
        cx.notify();
    }

    fn toggle(
        &mut self,
        section_idx: usize,
        entry_idx: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entry = &SETTINGS_CATALOG[section_idx].entries[entry_idx];
        let current = (entry.get)(&self.config) == "true";
        self.apply(section_idx, entry_idx, (!current).to_string(), window, cx);
    }

    fn choose(
        &mut self,
        section_idx: usize,
        entry_idx: usize,
        option: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply(section_idx, entry_idx, option, window, cx);
    }

    fn commit_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((si, ei)) = self.editing.take() {
            let value = self.edit_buffer.clone();
            self.apply(si, ei, value, window, cx);
        }
    }

    /// Set the entry's value on the in-memory config, persist it, and - for
    /// hot-appliable entries - re-apply the visual change immediately.
    fn apply(
        &mut self,
        section_idx: usize,
        entry_idx: usize,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let section = &SETTINGS_CATALOG[section_idx];
        let entry = &section.entries[entry_idx];
        (entry.set)(&mut self.config, &value);

        match config::write_back(section.key, entry.key, &value) {
            Ok(()) if entry.hot => self.hint = "saved - applied".to_string(),
            Ok(()) => self.hint = "saved - applies next time this pane opens".to_string(),
            Err(e) => self.hint = format!("could not save: {e}"),
        }

        if entry.hot {
            super::app::apply_config(&self.config, cx);
            if section.key == "theme" {
                let appearance = if self.config.theme.is_glass() {
                    WindowBackgroundAppearance::Blurred
                } else {
                    WindowBackgroundAppearance::Opaque
                };
                window.set_background_appearance(appearance);
                cx.set_global(GlassMode(self.config.theme.is_glass()));
            }
        }
        cx.notify();
    }

    fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_none() {
            return;
        }
        let ks = &e.keystroke;
        match ks.key.as_str() {
            "enter" => self.commit_edit(window, cx),
            "escape" => {
                self.editing = None;
                cx.notify();
            }
            "backspace" => {
                self.edit_buffer.pop();
                cx.notify();
            }
            _ => {
                if !ks.modifiers.platform && !ks.modifiers.control {
                    if let Some(ch) = &ks.key_char {
                        if !ch.is_empty() && !ch.chars().any(|c| c.is_control()) {
                            self.edit_buffer.push_str(ch);
                            cx.notify();
                        }
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- render

    fn rail(&self, cx: &Context<Self>) -> impl IntoElement {
        let selected = self.section_idx;
        div()
            .v_flex()
            .w(gpui::px(200.0))
            .flex_shrink_0()
            .h_full()
            .bg(chrome_bg(cx))
            .border_r_1()
            .border_color(glass_edge(cx))
            .child(
                div()
                    .px_3()
                    .pt_3()
                    .pb_1()
                    .text_xs()
                    .font_semibold()
                    .text_color(cx.theme().muted_foreground)
                    .child("SETTINGS"),
            )
            .child(
                div()
                    .v_flex()
                    .children(SETTINGS_CATALOG.iter().enumerate().map(|(i, s)| {
                        section_row(s.label, i == selected, ("settings-section", i), cx)
                    }))
                    .child(section_row(
                        "Integrations",
                        selected == integrations_idx(),
                        ("settings-section", integrations_idx()),
                        cx,
                    )),
            )
    }

    fn detail(&self, cx: &Context<Self>) -> impl IntoElement {
        let base = div()
            .id("settings-content")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p_4()
            .v_flex()
            .gap_1()
            .bg(pane_bg(cx));

        if self.section_idx == integrations_idx() {
            return base.child(integrations_panel(cx));
        }

        let Some(section) = SETTINGS_CATALOG.get(self.section_idx) else {
            return base;
        };

        let mut col = base;
        if !self.hint.is_empty() {
            col = col.child(
                div()
                    .pb_2()
                    .text_xs()
                    .text_color(cx.theme().success)
                    .child(self.hint.clone()),
            );
        }
        col.children(section.entries.iter().enumerate().map(|(i, entry)| {
            entry_row(
                self.section_idx,
                i,
                entry,
                &self.config,
                self.editing,
                &self.edit_buffer,
                cx,
            )
        }))
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus)
            .key_context("Settings")
            .on_key_down(cx.listener(Self::on_key))
            .h_flex()
            .size_full()
            .child(self.rail(cx))
            .child(self.detail(cx))
    }
}

fn section_row(
    label: &str,
    active: bool,
    id: (&'static str, usize),
    cx: &Context<SettingsView>,
) -> impl IntoElement {
    let idx = id.1;
    div()
        .id(id)
        .h_flex()
        .items_center()
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
        .child(label.to_string())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| this.select_section(idx, cx)),
        )
}

fn entry_row(
    section_idx: usize,
    entry_idx: usize,
    entry: &catalog::SettingEntry,
    config: &Config,
    editing: Option<(usize, usize)>,
    edit_buffer: &str,
    cx: &Context<SettingsView>,
) -> impl IntoElement {
    let current = (entry.get)(config);
    let is_editing = editing == Some((section_idx, entry_idx));

    let control: gpui::AnyElement = match &entry.kind {
        SettingKind::Toggle => Button::new(("setting-toggle", entry_idx))
            .label(if current == "true" { "On" } else { "Off" })
            .when(current == "true", |b| b.primary())
            .when(current != "true", |b| b.ghost())
            .small()
            .on_click(cx.listener(move |this, _, window, cx| {
                this.toggle(section_idx, entry_idx, window, cx)
            }))
            .into_any_element(),
        SettingKind::Choice(options) => div()
            .h_flex()
            .gap_1()
            .children(options.iter().map(|opt| {
                let active = *opt == current;
                let opt = opt.to_string();
                Button::new(format!("setting-choice-{entry_idx}-{opt}"))
                    .label(opt.clone())
                    .when(active, |b| b.primary())
                    .when(!active, |b| b.ghost())
                    .small()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.choose(section_idx, entry_idx, opt.clone(), window, cx)
                    }))
            }))
            .into_any_element(),
        SettingKind::Text | SettingKind::Number => div()
            .id(("setting-field", entry_idx))
            .px_2()
            .py_0p5()
            .rounded_md()
            .cursor_pointer()
            .border_1()
            .border_color(cx.theme().border)
            .text_sm()
            .text_color(cx.theme().foreground)
            .min_w(gpui::px(160.0))
            .child(if is_editing {
                format!("{edit_buffer}\u{2588}")
            } else if current.is_empty() {
                "(default)".to_string()
            } else {
                current.clone()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| this.start_edit(section_idx, entry_idx, cx)),
            )
            .into_any_element(),
    };

    div()
        .v_flex()
        .gap_1()
        .pb_3()
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
                        .child(entry.label),
                )
                .child(control),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(entry.description),
        )
}

/// Read-only pointers to the backend-integration panes (Memory/Trading/VCS/
/// Jobs/Self-extension), so this really is the map of everything the app can
/// do, per "settings guide everywhere."
fn integrations_panel(cx: &Context<SettingsView>) -> impl IntoElement {
    let rows: &[(&str, &str)] = &[
        (
            "Memory",
            "lifeos-memory - hybrid recall, consolidation, tiers. Real backend, thin client.",
        ),
        (
            "Trading",
            "Read-only broker positions - no order route exists in lifeos-api at all.",
        ),
        (
            "VCS",
            "lifeos-vcs - history/diff/branches/tags over any file type.",
        ),
        (
            "Jobs & Pipelines",
            "Real jobs queue + pipeline DAG registry. Actions status is not \
            available: lifeos-actions isn't linked into lifeos-api's HTTP surface.",
        ),
        (
            "Self-extension",
            "Ask-AI-to-add-a-module requests via /api/module-request; the cloud \
            side only enqueues, the scaffold build happens in the harness.",
        ),
    ];
    div()
        .v_flex()
        .gap_3()
        .children(rows.iter().map(|(title, desc)| {
            div()
                .v_flex()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(cx.theme().foreground)
                        .child(*title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(*desc),
                )
        }))
}
