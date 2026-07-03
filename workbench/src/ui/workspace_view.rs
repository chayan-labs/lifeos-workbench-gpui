//! Root workspace view: the IDE chrome shell.
//!
//! A custom `TitleBar` (brand + center-mode strip), a resizable file sidebar,
//! a vertically-resizable center (surface over terminal dock), and a
//! `StatusBar` - all themed via `gpui-component` and mouse-driven. Commands
//! arrive as [`actions`](super::actions) from the menu, the keymap, or the
//! in-window buttons; the handlers here mutate view state and re-render.
//!
//! The center surface is still a labelled placeholder per mode; the terminal
//! element, editor, and Life OS views replace it in later steps.

use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::resizable::{h_resizable, resizable_panel, v_resizable};
use gpui_component::status_bar::StatusBar;
use gpui_component::{ActiveTheme, Sizable, StyledExt, TitleBar};

use super::actions::{
    CommandPalette, FocusAgent, FocusEditor, FocusTerminal, OpenLifeOs, OpenRecall, ToggleDock,
    ToggleSidebar,
};

/// Which surface the center currently shows. Real content arrives per mode in
/// later steps; for now each renders a labelled placeholder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Editor,
    Terminal,
    Agent,
    LifeOs,
    Recall,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Editor => "Editor",
            Mode::Terminal => "Terminal",
            Mode::Agent => "Agent",
            Mode::LifeOs => "Life OS",
            Mode::Recall => "Recall",
        }
    }
}

/// The root view installed under `gpui_component::Root`.
pub struct WorkspaceView {
    sidebar_open: bool,
    dock_open: bool,
    mode: Mode,
    status_hint: String,
}

impl WorkspaceView {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            sidebar_open: true,
            dock_open: true,
            mode: Mode::Editor,
            status_hint: "ready".to_string(),
        }
    }

    // ---- action handlers (dispatched by menu / keymap / buttons) ----

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_open = !self.sidebar_open;
        self.status_hint = format!("sidebar {}", on_off(self.sidebar_open));
        cx.notify();
    }

    fn toggle_dock(&mut self, _: &ToggleDock, _: &mut Window, cx: &mut Context<Self>) {
        self.dock_open = !self.dock_open;
        self.status_hint = format!("terminal dock {}", on_off(self.dock_open));
        cx.notify();
    }

    fn focus_editor(&mut self, _: &FocusEditor, _: &mut Window, cx: &mut Context<Self>) {
        self.set_mode(Mode::Editor, cx);
    }
    fn focus_terminal(&mut self, _: &FocusTerminal, _: &mut Window, cx: &mut Context<Self>) {
        self.dock_open = true;
        self.set_mode(Mode::Terminal, cx);
    }
    fn focus_agent(&mut self, _: &FocusAgent, _: &mut Window, cx: &mut Context<Self>) {
        self.set_mode(Mode::Agent, cx);
    }
    fn open_lifeos(&mut self, _: &OpenLifeOs, _: &mut Window, cx: &mut Context<Self>) {
        self.set_mode(Mode::LifeOs, cx);
    }
    fn open_recall(&mut self, _: &OpenRecall, _: &mut Window, cx: &mut Context<Self>) {
        self.set_mode(Mode::Recall, cx);
    }
    fn command_palette(&mut self, _: &CommandPalette, _: &mut Window, cx: &mut Context<Self>) {
        self.status_hint = "command palette (todo)".to_string();
        cx.notify();
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        self.status_hint = mode.label().to_lowercase();
        cx.notify();
    }

    // ---- chrome regions ----

    /// Custom title bar: brand on the left, the center-mode strip on the right.
    fn title_bar(&self, _cx: &Context<Self>) -> impl IntoElement {
        let mode = self.mode;
        TitleBar::new()
            .child(div().font_semibold().child("Life OS Workbench"))
            .child(
                div()
                    .h_flex()
                    .gap_1()
                    .child(mode_button(
                        "m-editor",
                        "Editor",
                        mode == Mode::Editor,
                        FocusEditor,
                    ))
                    .child(mode_button(
                        "m-terminal",
                        "Terminal",
                        mode == Mode::Terminal,
                        FocusTerminal,
                    ))
                    .child(mode_button(
                        "m-agent",
                        "Agent",
                        mode == Mode::Agent,
                        FocusAgent,
                    ))
                    .child(mode_button(
                        "m-lifeos",
                        "Life OS",
                        mode == Mode::LifeOs,
                        OpenLifeOs,
                    ))
                    .child(mode_button(
                        "m-recall",
                        "Recall",
                        mode == Mode::Recall,
                        OpenRecall,
                    )),
            )
    }

    /// The left file sidebar (placeholder tree for now).
    fn sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .p_3()
            .gap_1()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("EXPLORER"),
            )
            .child(div().child("workbench/"))
            .child(div().child("services/"))
            .child(div().child("modules/"))
    }

    /// The center surface for the active mode (labelled placeholder for now).
    fn surface(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .bg(cx.theme().background)
            .text_color(cx.theme().muted_foreground)
            .child(
                div()
                    .text_color(cx.theme().foreground)
                    .font_semibold()
                    .child(self.mode.label()),
            )
            .child("surface renders here")
    }

    /// The bottom terminal dock (placeholder for now; real terminal in #22).
    fn dock(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .p_3()
            .bg(cx.theme().secondary)
            .border_t_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().muted_foreground)
            .child(div().text_xs().child("TERMINAL"))
            .child("shell mounts here")
    }

    /// The center column: surface over an optional resizable terminal dock.
    fn center(&self, cx: &Context<Self>) -> impl IntoElement {
        v_resizable("workspace-rows")
            .child(resizable_panel().child(self.surface(cx)))
            .when(self.dock_open, |group| {
                group.child(resizable_panel().size(px(220.0)).child(self.dock(cx)))
            })
    }

    /// Sidebar | center, both resizable via draggable dividers.
    fn body(&self, cx: &Context<Self>) -> impl IntoElement {
        h_resizable("workspace-cols")
            .when(self.sidebar_open, |group| {
                group.child(
                    resizable_panel()
                        .size(px(240.0))
                        .size_range(px(180.0)..px(420.0))
                        .child(self.sidebar(cx)),
                )
            })
            .child(resizable_panel().child(self.center(cx)))
    }

    fn status_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        StatusBar::new()
            .left(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.status_hint.clone()),
            )
            .right(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} · main", self.mode.label())),
            )
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("workbench")
            .v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            // Commands from the menu / keymap route here.
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::toggle_dock))
            .on_action(cx.listener(Self::focus_editor))
            .on_action(cx.listener(Self::focus_terminal))
            .on_action(cx.listener(Self::focus_agent))
            .on_action(cx.listener(Self::open_lifeos))
            .on_action(cx.listener(Self::open_recall))
            .on_action(cx.listener(Self::command_palette))
            .child(self.title_bar(cx))
            .child(div().flex_1().min_h_0().w_full().child(self.body(cx)))
            .child(self.status_bar(cx))
    }
}

/// A title-bar mode button that dispatches `action` on click.
fn mode_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    action: impl gpui::Action + Clone,
) -> Button {
    let button = Button::new(id).label(label).small();
    let button = if active {
        button.primary()
    } else {
        button.ghost()
    };
    button.on_click(move |_, window, cx| window.dispatch_action(Box::new(action.clone()), cx))
}

fn on_off(v: bool) -> &'static str {
    if v {
        "shown"
    } else {
        "hidden"
    }
}
