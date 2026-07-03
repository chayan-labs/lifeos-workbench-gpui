//! Native macOS menu bar.
//!
//! Built from the same [`actions`](super::actions) the buttons and keymap use,
//! so a menu click, a button click, and a shortcut all dispatch the identical
//! action. Installed once at startup via `cx.set_menus`; the OS renders it as
//! the real top-of-screen menu bar, and each item shows its bound accelerator.

use gpui::{App, Menu, MenuItem};

use super::actions::{
    About, CloseTab, ClosePane, CommandPalette, FocusAgent, FocusEditor, FocusNextPane,
    FocusPrevPane, FocusTerminal, NewTab, OpenFile, OpenLifeOs, OpenRecall, Quit, SplitDown,
    SplitRight, ToggleDock, ToggleSidebar,
};

/// Install the application menu bar.
pub fn install(cx: &mut App) {
    cx.set_menus(build());
}

fn build() -> Vec<Menu> {
    vec![
        Menu {
            name: "Life OS Workbench".into(),
            items: vec![
                MenuItem::action("About Life OS Workbench", About),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ],
            disabled: false,
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open File...", OpenFile),
                MenuItem::action("New Tab", NewTab),
                MenuItem::separator(),
                MenuItem::action("Close Tab", CloseTab),
            ],
            disabled: false,
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Command Palette", CommandPalette),
                MenuItem::separator(),
                MenuItem::action("Toggle Sidebar", ToggleSidebar),
                MenuItem::action("Toggle Terminal Dock", ToggleDock),
            ],
            disabled: false,
        },
        Menu {
            name: "Pane".into(),
            items: vec![
                MenuItem::action("Split Right", SplitRight),
                MenuItem::action("Split Down", SplitDown),
                MenuItem::separator(),
                MenuItem::action("Focus Next", FocusNextPane),
                MenuItem::action("Focus Previous", FocusPrevPane),
                MenuItem::action("Close Pane", ClosePane),
            ],
            disabled: false,
        },
        Menu {
            name: "Go".into(),
            items: vec![
                MenuItem::action("Editor", FocusEditor),
                MenuItem::action("Terminal", FocusTerminal),
                MenuItem::action("Agent", FocusAgent),
            ],
            disabled: false,
        },
        Menu {
            name: "Life OS".into(),
            items: vec![
                MenuItem::action("Modules", OpenLifeOs),
                MenuItem::action("Recall Search", OpenRecall),
            ],
            disabled: false,
        },
    ]
}
