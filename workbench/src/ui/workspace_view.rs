//! Root workspace view: the IDE chrome shell.
//!
//! A custom `TitleBar` (brand + center-mode strip), a clickable tab strip, a
//! resizable file-tree sidebar, a center that tiles the active tab's pane tree,
//! an optional terminal dock, and a `StatusBar` - all themed via
//! `gpui-component` and mouse-driven. Every command (menu, keymap, buttons,
//! palette) funnels through the single [`WorkspaceView::run_command`], so a
//! shortcut, a click, and a palette pick share one code path.
//!
//! Pane leaves show a labelled placeholder for the active mode for now; the
//! editor, agent, and Life OS views replace that content in later steps. The
//! tiling engine, tabs, sidebar, and palette are real.

use std::path::PathBuf;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, AnyElement, AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::resizable::{h_resizable, resizable_panel, v_resizable};
use gpui_component::status_bar::StatusBar;
use gpui_component::{ActiveTheme, Sizable, StyledExt, TitleBar};

use super::actions::{
    About, CloseTab, ClosePane, CommandPalette, FocusAgent, FocusEditor, FocusNextPane,
    FocusPrevPane, FocusTerminal, NewTab, OpenFile, OpenLifeOs, OpenRecall, SplitDown, SplitRight,
    ToggleDock, ToggleSidebar,
};
use super::commands::{commands, filter, CommandId};
use super::file_tree::FileTree;
use super::panes::{Layout, LayoutNode, PaneId, SplitDir};
use super::terminal::TerminalView;

/// Which surface a pane currently shows. Real content arrives per mode in later
/// steps; for now each leaf renders a labelled placeholder.
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
    terminal: Entity<TerminalView>,
    layout: Layout,
    file_tree: FileTree,
    selected_file: Option<PathBuf>,
    // Command palette (inline overlay state).
    palette_open: bool,
    palette_query: String,
    palette_selected: usize,
    palette_focus: FocusHandle,
}

impl WorkspaceView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let terminal = cx.new(|cx| TerminalView::new(window, cx));
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            sidebar_open: true,
            dock_open: true,
            mode: Mode::Editor,
            status_hint: "ready".to_string(),
            terminal,
            layout: Layout::new(),
            file_tree: FileTree::open(&root),
            selected_file: None,
            palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
            palette_focus: cx.focus_handle(),
        }
    }

    // ---- the one command handler everything funnels through ----

    fn run_command(&mut self, id: CommandId, window: &mut Window, cx: &mut Context<Self>) {
        use CommandId::*;
        match id {
            SplitRight => {
                self.layout = self.layout.split_focused(SplitDir::Horizontal).0;
                self.hint("split right");
            }
            SplitDown => {
                self.layout = self.layout.split_focused(SplitDir::Vertical).0;
                self.hint("split down");
            }
            ClosePane => match self.layout.close_focused() {
                Some(next) => {
                    self.layout = next;
                    self.hint("closed pane");
                }
                None => self.hint("last pane - use Quit to exit"),
            },
            FocusNext => {
                self.layout = self.layout.focus_next();
                self.hint("focus next");
            }
            FocusPrev => {
                self.layout = self.layout.focus_prev();
                self.hint("focus previous");
            }
            NewTab => {
                self.layout = self.layout.new_tab().0;
                self.hint("new tab");
            }
            NextTab => {
                self.layout = self.layout.next_tab();
                self.hint("next tab");
            }
            ToggleEditor => {
                self.mode = if self.mode == Mode::Editor {
                    Mode::Terminal
                } else {
                    Mode::Editor
                };
                self.hint(self.mode.label());
            }
            FocusEditor => self.set_mode(Mode::Editor),
            FocusTerminal => {
                self.dock_open = true;
                self.set_mode(Mode::Terminal);
                let handle = self.terminal.read(cx).handle();
                window.focus(&handle, cx);
            }
            ToggleSidebar => {
                self.sidebar_open = !self.sidebar_open;
                self.hint(&format!("sidebar {}", shown(self.sidebar_open)));
            }
            ToggleDock => {
                self.dock_open = !self.dock_open;
                self.hint(&format!("terminal dock {}", shown(self.dock_open)));
            }
            OpenFilePicker => self.hint("fuzzy picker (todo)"),
            OpenAgentPane => self.set_mode(Mode::Agent),
            OpenSearchPane => self.set_mode(Mode::Recall),
            OpenLifeOsPane => self.set_mode(Mode::LifeOs),
            Quit => window.dispatch_action(Box::new(super::actions::Quit), cx),
        }
        cx.notify();
    }

    fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.status_hint = mode.label().to_lowercase();
    }

    fn hint(&mut self, msg: &str) {
        self.status_hint = msg.to_string();
    }

    // ---- action wrappers (menu / keymap / buttons all dispatch these) ----

    fn on_toggle_sidebar(&mut self, _: &ToggleSidebar, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::ToggleSidebar, w, cx);
    }
    fn on_toggle_dock(&mut self, _: &ToggleDock, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::ToggleDock, w, cx);
    }
    fn on_focus_editor(&mut self, _: &FocusEditor, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::FocusEditor, w, cx);
    }
    fn on_focus_terminal(&mut self, _: &FocusTerminal, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::FocusTerminal, w, cx);
    }
    fn on_focus_agent(&mut self, _: &FocusAgent, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::OpenAgentPane, w, cx);
    }
    fn on_open_lifeos(&mut self, _: &OpenLifeOs, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::OpenLifeOsPane, w, cx);
    }
    fn on_open_recall(&mut self, _: &OpenRecall, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::OpenSearchPane, w, cx);
    }
    fn on_new_tab(&mut self, _: &NewTab, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::NewTab, w, cx);
    }
    fn on_close_tab(&mut self, _: &CloseTab, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::ClosePane, w, cx);
    }
    fn on_split_right(&mut self, _: &SplitRight, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::SplitRight, w, cx);
    }
    fn on_split_down(&mut self, _: &SplitDown, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::SplitDown, w, cx);
    }
    fn on_close_pane(&mut self, _: &ClosePane, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::ClosePane, w, cx);
    }
    fn on_focus_next_pane(&mut self, _: &FocusNextPane, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::FocusNext, w, cx);
    }
    fn on_focus_prev_pane(&mut self, _: &FocusPrevPane, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::FocusPrev, w, cx);
    }
    fn on_open_file(&mut self, _: &OpenFile, w: &mut Window, cx: &mut Context<Self>) {
        self.run_command(CommandId::OpenFilePicker, w, cx);
    }
    fn on_about(&mut self, _: &About, _: &mut Window, cx: &mut Context<Self>) {
        self.hint("Life OS Workbench - GPU-native");
        cx.notify();
    }

    // ---- command palette ----

    fn on_command_palette(&mut self, _: &CommandPalette, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(window, cx);
    }

    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = true;
        self.palette_query.clear();
        self.palette_selected = 0;
        window.focus(&self.palette_focus, cx);
        cx.notify();
    }

    fn close_palette(&mut self, cx: &mut Context<Self>) {
        self.palette_open = false;
        cx.notify();
    }

    fn palette_matches(&self) -> Vec<super::commands::Command> {
        filter(&self.palette_query, &commands())
    }

    fn on_palette_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &e.keystroke;
        match ks.key.as_str() {
            "escape" => self.close_palette(cx),
            "enter" => self.palette_run_selected(window, cx),
            "up" => {
                self.palette_selected = self.palette_selected.saturating_sub(1);
                cx.notify();
            }
            "down" => {
                let n = self.palette_matches().len();
                if n > 0 {
                    self.palette_selected = (self.palette_selected + 1).min(n - 1);
                }
                cx.notify();
            }
            "backspace" => {
                self.palette_query.pop();
                self.palette_selected = 0;
                cx.notify();
            }
            _ => {
                // Printable character (ignore modifier chords).
                if !ks.modifiers.platform && !ks.modifiers.control {
                    if let Some(ch) = &ks.key_char {
                        if !ch.is_empty() && !ch.chars().any(|c| c.is_control()) {
                            self.palette_query.push_str(ch);
                            self.palette_selected = 0;
                            cx.notify();
                        }
                    }
                }
            }
        }
    }

    fn palette_run_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let matches = self.palette_matches();
        let picked = matches.get(self.palette_selected).map(|c| c.id);
        self.palette_open = false;
        if let Some(id) = picked {
            self.run_command(id, window, cx);
        } else {
            cx.notify();
        }
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
                    .child(mode_button("m-editor", "Editor", mode == Mode::Editor, FocusEditor))
                    .child(mode_button(
                        "m-terminal",
                        "Terminal",
                        mode == Mode::Terminal,
                        FocusTerminal,
                    ))
                    .child(mode_button("m-agent", "Agent", mode == Mode::Agent, FocusAgent))
                    .child(mode_button("m-lifeos", "Life OS", mode == Mode::LifeOs, OpenLifeOs))
                    .child(mode_button("m-recall", "Recall", mode == Mode::Recall, OpenRecall)),
            )
    }

    /// The clickable tab strip: one chip per tab, active highlighted, plus a
    /// new-tab button.
    fn tab_strip(&self, cx: &Context<Self>) -> impl IntoElement {
        let active = self.layout.active_tab;
        div()
            .h_flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .bg(cx.theme().secondary)
            .border_b_1()
            .border_color(cx.theme().border)
            .children(self.layout.tabs.iter().enumerate().map(|(i, _)| {
                let is_active = i == active;
                div()
                    .id(("tab", i))
                    .px_3()
                    .py_0p5()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if is_active {
                        cx.theme().background
                    } else {
                        cx.theme().secondary
                    })
                    .text_color(if is_active {
                        cx.theme().foreground
                    } else {
                        cx.theme().muted_foreground
                    })
                    .text_xs()
                    .child(format!("Tab {}", i + 1))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.layout = this.layout.switch_tab(i);
                            this.hint(&format!("tab {}", i + 1));
                            cx.notify();
                        }),
                    )
            }))
            .child(
                div()
                    .id("tab-new")
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(cx.theme().muted_foreground)
                    .text_xs()
                    .child("+")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.run_command(CommandId::NewTab, window, cx);
                        }),
                    ),
            )
    }

    /// The left file sidebar: the real directory tree over [`FileTree`].
    fn sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let root_name = self
            .file_tree
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string();
        let rows = self.file_tree.rows();
        let selected = self.file_tree.selected;

        div()
            .v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("EXPLORER - {root_name}")),
            )
            .child(
                div()
                    .id("file-tree")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows.into_iter().enumerate().map(|(i, row)| {
                        let is_sel = i == selected;
                        let indent = px(8.0 + row.depth as f32 * 12.0);
                        let icon = if row.is_dir {
                            if row.expanded {
                                "\u{25BE} " // ▾
                            } else {
                                "\u{25B8} " // ▸
                            }
                        } else {
                            "  "
                        };
                        let path = row.path.clone();
                        let is_dir = row.is_dir;
                        div()
                            .id(("row", i))
                            .h_flex()
                            .items_center()
                            .w_full()
                            .pl(indent)
                            .pr_2()
                            .py_0p5()
                            .text_sm()
                            .cursor_pointer()
                            .when(is_sel, |d| d.bg(cx.theme().accent))
                            .text_color(if row.is_dir {
                                cx.theme().foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(format!("{icon}{}", row.name()))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.file_tree = this.file_tree.selected(i);
                                    if is_dir {
                                        this.file_tree = this.file_tree.toggled(&path);
                                    } else {
                                        this.selected_file = Some(path.clone());
                                        let rel = path
                                            .strip_prefix(&this.file_tree.root)
                                            .unwrap_or(&path)
                                            .display()
                                            .to_string();
                                        this.hint(&format!("open: {rel} (editor in #24)"));
                                    }
                                    cx.notify();
                                }),
                            )
                    })),
            )
    }

    /// Recursively tile a tab's pane tree. Splits become flex rows/columns with
    /// a 1px divider; leaves render the mode placeholder with a focus ring.
    ///
    /// Each half is sized with an explicit `flex_basis(50%)` rather than
    /// `flex_grow` + `size_full`: percentage height does not distribute in a
    /// column flex without a definite basis, which otherwise collapses the
    /// first pane of a vertical split to zero.
    fn render_pane_tree(&self, node: &LayoutNode, cx: &Context<Self>) -> AnyElement {
        match node {
            LayoutNode::Leaf(id) => self.render_leaf(*id, cx),
            LayoutNode::Split { dir, first, second } => {
                let a = self.render_pane_tree(first, cx);
                let b = self.render_pane_tree(second, cx);
                let base = div().size_full();
                let base = match dir {
                    SplitDir::Horizontal => base.h_flex(),
                    SplitDir::Vertical => base.v_flex(),
                };
                let half = || {
                    div()
                        .flex_basis(gpui::relative(0.5))
                        .flex_shrink_1()
                        .min_w_0()
                        .min_h_0()
                        .overflow_hidden()
                };
                let divider = match dir {
                    SplitDir::Horizontal => div().w(px(1.0)).h_full(),
                    SplitDir::Vertical => div().h(px(1.0)).w_full(),
                }
                .flex_shrink_0()
                .bg(cx.theme().border);
                base.child(half().child(a))
                    .child(divider)
                    .child(half().child(b))
                    .into_any_element()
            }
        }
    }

    fn render_leaf(&self, id: PaneId, cx: &Context<Self>) -> AnyElement {
        let focused = self.layout.tab().focused == id;
        let border = if focused {
            cx.theme().caret
        } else {
            cx.theme().border
        };
        div()
            .id(("pane", id as usize))
            .v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .border_1()
            .border_color(border)
            .bg(cx.theme().background)
            .text_color(cx.theme().muted_foreground)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.layout = this.layout.focus_pane(id);
                    cx.notify();
                }),
            )
            .child(
                div()
                    .text_color(cx.theme().foreground)
                    .font_semibold()
                    .child(self.mode.label()),
            )
            .child(div().text_xs().child(format!("pane {id}")))
            .into_any_element()
    }

    /// The bottom terminal dock: a header strip over the live terminal view.
    fn dock(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .bg(cx.theme().secondary)
                    .text_color(cx.theme().muted_foreground)
                    .child("TERMINAL"),
            )
            .child(div().flex_1().min_h_0().child(self.terminal.clone()))
    }

    /// The center column: the pane tree over an optional resizable dock.
    fn center(&self, cx: &Context<Self>) -> impl IntoElement {
        let tree = self.render_pane_tree(&self.layout.tab().root, cx);
        v_resizable("workspace-rows")
            .child(resizable_panel().child(div().size_full().child(tree)))
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
        let panes = self.layout.tab().root.panes().len();
        StatusBar::new()
            .left(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.status_hint.clone()),
            )
            .right(
                div().text_xs().text_color(cx.theme().muted_foreground).child(format!(
                    "{} · {} pane{} · main",
                    self.mode.label(),
                    panes,
                    if panes == 1 { "" } else { "s" }
                )),
            )
    }

    /// The command palette overlay: a scrim + centered modal with a fuzzy list.
    fn palette_overlay(&self, cx: &Context<Self>) -> impl IntoElement {
        let matches = self.palette_matches();
        let selected = self
            .palette_selected
            .min(matches.len().saturating_sub(1));

        let modal = div()
            .track_focus(&self.palette_focus)
            .on_key_down(cx.listener(Self::on_palette_key))
            .w(px(560.0))
            .max_h(px(420.0))
            .mt(px(84.0))
            .v_flex()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .overflow_hidden()
            // Clicks inside the modal must not fall through to the scrim.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_color(cx.theme().foreground)
                    .child(format!("\u{203A} {}\u{2588}", self.palette_query)),
            )
            .child(
                div()
                    .id("palette-list")
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(matches.into_iter().enumerate().map(|(i, cmd)| {
                        let is_sel = i == selected;
                        div()
                            .id(("cmd", i))
                            .px_3()
                            .py_1()
                            .w_full()
                            .cursor_pointer()
                            .text_sm()
                            .when(is_sel, |d| d.bg(cx.theme().accent))
                            .text_color(if is_sel {
                                cx.theme().foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(cmd.title)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.palette_selected = i;
                                    this.palette_run_selected(window, cx);
                                }),
                            )
                    })),
            );

        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_start()
            .bg(gpui::black().opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.close_palette(cx)),
            )
            .child(modal)
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main = div()
            .v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.title_bar(cx))
            .child(self.tab_strip(cx))
            .child(div().flex_1().min_h_0().w_full().child(self.body(cx)))
            .child(self.status_bar(cx));

        div()
            .id("workbench")
            .relative()
            .size_full()
            // Commands from the menu / keymap route here (ancestor of both the
            // main column and the palette overlay, so shortcuts work in both).
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_toggle_dock))
            .on_action(cx.listener(Self::on_focus_editor))
            .on_action(cx.listener(Self::on_focus_terminal))
            .on_action(cx.listener(Self::on_focus_agent))
            .on_action(cx.listener(Self::on_open_lifeos))
            .on_action(cx.listener(Self::on_open_recall))
            .on_action(cx.listener(Self::on_new_tab))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_split_right))
            .on_action(cx.listener(Self::on_split_down))
            .on_action(cx.listener(Self::on_close_pane))
            .on_action(cx.listener(Self::on_focus_next_pane))
            .on_action(cx.listener(Self::on_focus_prev_pane))
            .on_action(cx.listener(Self::on_open_file))
            .on_action(cx.listener(Self::on_about))
            .on_action(cx.listener(Self::on_command_palette))
            .child(main)
            .when(self.palette_open, |d| d.child(self.palette_overlay(cx)))
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

fn shown(v: bool) -> &'static str {
    if v {
        "shown"
    } else {
        "hidden"
    }
}
