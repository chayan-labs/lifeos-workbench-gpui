//! The settings catalog: every field in [`Config`]/`LuaConfig`, with a
//! plain-English description, so this doubles as documentation and a live
//! editor - the "full setting guide" - rather than just a reference page.

use crate::ui::config::{Config, EditorEngine, ThemePref};

/// What kind of control an entry renders.
pub enum SettingKind {
    Toggle,
    /// A fixed set of choices; the pane renders one small button per option.
    Choice(&'static [&'static str]),
    Text,
    Number,
}

pub struct SettingEntry {
    /// The `config.lua` key within its section's sub-table.
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub kind: SettingKind,
    pub get: fn(&Config) -> String,
    pub set: fn(&mut Config, &str),
    /// Whether changing this takes effect immediately (theme mode, fonts) or
    /// only the next time the affected pane is (re)opened (editor engine,
    /// agent command) - shown as an honest hint rather than pretended live.
    pub hot: bool,
}

pub struct SettingSection {
    /// The `config.lua` top-level section table name.
    pub key: &'static str,
    pub label: &'static str,
    pub entries: &'static [SettingEntry],
}

pub static SETTINGS_CATALOG: &[SettingSection] = &[
    SettingSection {
        key: "editor",
        label: "Editor",
        entries: &[
            SettingEntry {
                key: "engine",
                label: "Engine",
                description:
                    "Which engine backs the editor pane. \"native\" is gpui-component's \
                    built-in code editor (tree-sitter highlighting, folding, LSP-ready). \"helix\" \
                    is not yet wired - selecting it shows an honest placeholder instead of editing.",
                kind: SettingKind::Choice(&["native", "helix"]),
                get: |c| c.editor.engine.label().to_string(),
                set: |c, v| c.editor.engine = EditorEngine::parse(v),
                hot: false,
            },
            SettingEntry {
                key: "line_number",
                label: "Line numbers",
                description: "Show line numbers in the editor gutter.",
                kind: SettingKind::Toggle,
                get: |c| c.editor.line_number.to_string(),
                set: |c, v| c.editor.line_number = v == "true",
                hot: false,
            },
            SettingEntry {
                key: "indent_guides",
                label: "Indent guides",
                description: "Show vertical indent-guide lines in the editor.",
                kind: SettingKind::Toggle,
                get: |c| c.editor.indent_guides.to_string(),
                set: |c, v| c.editor.indent_guides = v == "true",
                hot: false,
            },
            SettingEntry {
                key: "soft_wrap",
                label: "Soft wrap",
                description: "Wrap long lines instead of scrolling horizontally.",
                kind: SettingKind::Toggle,
                get: |c| c.editor.soft_wrap.to_string(),
                set: |c, v| c.editor.soft_wrap = v == "true",
                hot: false,
            },
            SettingEntry {
                key: "tab_size",
                label: "Tab size",
                description: "Spaces per indent level (clamped to 1-16).",
                kind: SettingKind::Number,
                get: |c| c.editor.tab_size.to_string(),
                set: |c, v| {
                    if let Ok(n) = v.parse::<u8>() {
                        c.editor.tab_size = n.clamp(1, 16);
                    }
                },
                hot: false,
            },
        ],
    },
    SettingSection {
        key: "theme",
        label: "Theme",
        entries: &[SettingEntry {
            key: "mode",
            label: "Mode",
            description: "\"dark\" or \"light\", or \"glass\": a real macOS window-vibrancy \
                blur (not a fake) applied behind every pane in the app, not just the terminal.",
            kind: SettingKind::Choice(&["dark", "light", "glass"]),
            get: |c| {
                match c.theme {
                    ThemePref::Dark => "dark",
                    ThemePref::Light => "light",
                    ThemePref::Glass => "glass",
                }
                .to_string()
            },
            set: |c, v| c.theme = ThemePref::parse(v),
            hot: true,
        }],
    },
    SettingSection {
        key: "font",
        label: "Font",
        entries: &[
            SettingEntry {
                key: "family",
                label: "UI font family",
                description:
                    "Overrides the theme's default UI font. Leave blank to use the theme default.",
                kind: SettingKind::Text,
                get: |c| c.font.ui_family.clone().unwrap_or_default(),
                set: |c, v| c.font.ui_family = (!v.trim().is_empty()).then(|| v.trim().to_string()),
                hot: true,
            },
            SettingEntry {
                key: "mono_family",
                label: "Monospace font family",
                description: "Overrides the theme's default monospace/terminal font.",
                kind: SettingKind::Text,
                get: |c| c.font.mono_family.clone().unwrap_or_default(),
                set: |c, v| {
                    c.font.mono_family = (!v.trim().is_empty()).then(|| v.trim().to_string())
                },
                hot: true,
            },
            SettingEntry {
                key: "size",
                label: "UI font size",
                description: "Point size for UI text. Leave blank to use the theme default.",
                kind: SettingKind::Number,
                get: |c| c.font.ui_size.map(|s| s.to_string()).unwrap_or_default(),
                set: |c, v| c.font.ui_size = v.trim().parse::<f32>().ok(),
                hot: true,
            },
            SettingEntry {
                key: "mono_size",
                label: "Monospace font size",
                description: "Point size for monospace/terminal text.",
                kind: SettingKind::Number,
                get: |c| c.font.mono_size.map(|s| s.to_string()).unwrap_or_default(),
                set: |c, v| c.font.mono_size = v.trim().parse::<f32>().ok(),
                hot: true,
            },
        ],
    },
    SettingSection {
        key: "agent",
        label: "Agent",
        entries: &[SettingEntry {
            key: "command",
            label: "Agent command",
            description: "The ACP agent binary to spawn. Auto-detected from $PATH when unset \
                (candidates: claude-code-acp, claude, codex, gemini, in that order) - an explicit \
                value here always wins. Use the Agent pane's own \"Change...\" picker to browse \
                what's actually on $PATH.",
            kind: SettingKind::Text,
            get: |c| c.agent.command.clone().unwrap_or_default(),
            set: |c, v| c.agent.command = (!v.trim().is_empty()).then(|| v.trim().to_string()),
            hot: false,
        }],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_round_trips_through_its_own_getter_and_setter() {
        for section in SETTINGS_CATALOG {
            for entry in section.entries {
                let mut config = Config::default();
                let before = (entry.get)(&config);
                (entry.set)(&mut config, &before);
                let after = (entry.get)(&config);
                assert_eq!(
                    before, after,
                    "{}.{} did not round-trip",
                    section.key, entry.key
                );
            }
        }
    }

    #[test]
    fn glass_theme_is_reachable_from_the_catalog() {
        let theme_entry = &SETTINGS_CATALOG[1].entries[0];
        let mut config = Config::default();
        (theme_entry.set)(&mut config, "glass");
        assert!(config.theme.is_glass());
        assert!(theme_entry.hot, "theme changes should hot-apply");
    }
}
