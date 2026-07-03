//! The editor pane.
//!
//! Wraps gpui-component's native code editor ([`InputState`] in code-editor
//! mode: tree-sitter 0.26 highlighting, line numbers, indent guides, folding,
//! LSP-ready) behind an engine seam. The [`EditorEngine`] config selects the
//! backing engine; `Native` renders the real editor, `Helix` renders an honest
//! "not yet wired" panel until `helix-core`'s syntax layer is aligned to
//! tree-sitter 0.26 (tracked as a #24 follow-up). The wrapper owns file opening
//! (path -> language -> buffer) and exposes the focus handle so the workspace
//! can focus it.

pub mod lang;

use std::path::{Path, PathBuf};

use gpui::{
    div, App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, SharedString, Styled, Window,
};
use gpui_component::input::{Input, InputState, TabSize};
use gpui_component::{ActiveTheme, StyledExt};

use super::config::{EditorConfig, EditorEngine};

pub struct EditorView {
    config: EditorConfig,
    /// The native code editor state. Always constructed (cheap, empty buffer);
    /// only rendered when the engine is `Native`.
    state: Entity<InputState>,
    /// The file currently loaded, if any.
    path: Option<PathBuf>,
    /// The highlighter language name for the loaded file.
    language: SharedString,
}

impl EditorView {
    pub fn new(config: EditorConfig, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let opts = config;
        let state = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(lang::PLAIN)
                .line_number(opts.line_number)
                .indent_guides(opts.indent_guides)
                .soft_wrap(opts.soft_wrap)
                .tab_size(TabSize {
                    tab_size: opts.tab_size as usize,
                    hard_tabs: false,
                })
                .placeholder("Open a file from the sidebar to start editing.")
        });

        Self {
            config,
            state,
            path: None,
            language: SharedString::from(lang::PLAIN),
        }
    }

    /// Load `path` into the editor: read it, pick a highlighter language from
    /// its extension, and replace the buffer. Errors surface as the buffer
    /// contents so the failure is visible in-pane rather than swallowed.
    pub fn open_path(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let language = lang::language_for(path);
        let (content, language) = match std::fs::read_to_string(path) {
            Ok(text) => (text, language),
            Err(e) => (format!("// failed to open {}:\n// {e}", path.display()), lang::PLAIN),
        };

        self.path = Some(path.to_path_buf());
        self.language = SharedString::from(language);

        let lang_name = language.to_string();
        self.state.update(cx, |state, cx| {
            state.set_highlighter(lang_name, cx);
            state.set_value(content, window, cx);
        });
        cx.notify();
    }

    /// The path of the loaded file, if any.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// A short title for the tab / status line (file name, or a placeholder).
    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "untitled".to_string())
    }

    /// The focus handle of the underlying editor, so the workspace can focus
    /// the editor when the Editor mode is selected.
    pub fn handle(&self, cx: &App) -> FocusHandle {
        self.state.focus_handle(cx)
    }

    /// The honest placeholder for the not-yet-wired Helix engine.
    fn helix_placeholder(&self, cx: &Context<Self>) -> impl IntoElement {
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
                    .child("Helix engine"),
            )
            .child(div().text_xs().child(
                "not yet wired - pending helix-core syntax alignment to tree-sitter 0.26",
            ))
            .child(
                div()
                    .text_xs()
                    .child("set editor.engine = \"native\" (default) to edit"),
            )
    }
}

impl Focusable for EditorView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.focus_handle(cx)
    }
}

impl Render for EditorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.config.engine {
            EditorEngine::Native => div()
                .size_full()
                .bg(cx.theme().background)
                .child(
                    Input::new(&self.state)
                        .bordered(false)
                        .focus_bordered(false)
                        .h_full()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(cx.theme().mono_font_size),
                )
                .into_any_element(),
            EditorEngine::Helix => self.helix_placeholder(cx).into_any_element(),
        }
    }
}
