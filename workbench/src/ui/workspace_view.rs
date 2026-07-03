//! Root workspace view: the IDE chrome shell.
//!
//! For now this is the workspace *scaffold* - a title bar, a file sidebar, a
//! center body, and a statusline - laid out with real GUI panels and themed
//! via `gpui-component`. Subsequent steps replace the placeholders with the
//! resizable dock, terminal element, editor, and Life OS views (CLAUDE.md
//! build order steps 2-6).

use gpui::{div, px, Context, IntoElement, ParentElement, Render, Styled, Window};
use gpui_component::button::Button;
use gpui_component::{ActiveTheme, StyledExt};

/// The root view installed under `gpui_component::Root`.
pub struct WorkspaceView {
    /// Placeholder for the transient "last action" hint shown in the status
    /// bar; proves state + re-render wiring ahead of the real command loop.
    status_hint: String,
}

impl WorkspaceView {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            status_hint: "ready".to_string(),
        }
    }

    /// The top chrome: app identity on the left, quick actions on the right.
    fn title_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .h(px(38.0))
            .w_full()
            .px_3()
            .items_center()
            .justify_between()
            .bg(cx.theme().title_bar)
            .border_b_1()
            .border_color(cx.theme().title_bar_border)
            .child(
                div()
                    .font_semibold()
                    .text_color(cx.theme().foreground)
                    .child("Life OS Workbench"),
            )
            .child(
                Button::new("cmd-palette")
                    .label("Command Palette")
                    .on_click(|_, _, _| {}),
            )
    }

    /// The left file sidebar (placeholder tree for now).
    fn sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .w(px(240.0))
            .h_full()
            .p_3()
            .gap_2()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
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

    /// The center body: editor / terminal / Life OS will dock here.
    fn body(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .items_center()
            .justify_center()
            .gap_2()
            .bg(cx.theme().background)
            .text_color(cx.theme().muted_foreground)
            .child(div().text_color(cx.theme().foreground).child("Workspace"))
            .child("editor · terminal · agent · Life OS dock here")
    }

    /// The bottom statusline.
    fn status_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .h(px(24.0))
            .w_full()
            .px_3()
            .items_center()
            .justify_between()
            .bg(cx.theme().status_bar)
            .border_t_1()
            .border_color(cx.theme().status_bar_border)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(div().child(self.status_hint.clone()))
            .child(div().child("main"))
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.title_bar(cx))
            .child(
                div()
                    .h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.sidebar(cx))
                    .child(self.body(cx)),
            )
            .child(self.status_bar(cx))
    }
}
