//! Global gpui actions + key bindings.
//!
//! Actions are the single source of truth for commands: the native menu, the
//! in-window buttons, and the keymap all dispatch these, and the handlers live
//! in one place (`app.rs` for app-global, `WorkspaceView` for view-local). The
//! menu shows each item's accelerator by looking it up in the bound keymap, so
//! defining a binding here is what makes the shortcut appear in the menu.

use gpui::{actions, App, KeyBinding};

actions!(
    workbench,
    [
        /// Quit the application.
        Quit,
        /// Show the about panel.
        About,
        /// Open a file into the editor.
        OpenFile,
        /// Toggle the command palette.
        CommandPalette,
        /// Show/hide the file sidebar.
        ToggleSidebar,
        /// Show/hide the terminal dock.
        ToggleDock,
        /// Open a new tab.
        NewTab,
        /// Close the active tab.
        CloseTab,
        /// Focus the editor surface in the center.
        FocusEditor,
        /// Focus the integrated terminal.
        FocusTerminal,
        /// Focus the AI agent pane.
        FocusAgent,
        /// Open the Life OS module browser.
        OpenLifeOs,
        /// Open recall (semantic) search.
        OpenRecall,
    ]
);

/// Bind the default keymap. Bindings are global (no context) for now; per-pane
/// contexts arrive with the real panes. The menu renders these as accelerators.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-o", OpenFile, None),
        KeyBinding::new("cmd-p", CommandPalette, None),
        KeyBinding::new("cmd-k", CommandPalette, None),
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-j", ToggleDock, None),
        KeyBinding::new("cmd-t", NewTab, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("cmd-1", FocusEditor, None),
        KeyBinding::new("cmd-2", FocusTerminal, None),
        KeyBinding::new("cmd-3", FocusAgent, None),
        KeyBinding::new("cmd-l", OpenLifeOs, None),
        KeyBinding::new("cmd-shift-f", OpenRecall, None),
    ]);
}
