//! Runtime configuration for the gpui frontend.
//!
//! Config is layered, lowest precedence first: built-in defaults, then the
//! user's `config.lua` (evaluated with `mlua`), then a small set of environment
//! overrides for quick end-to-end testing. The Lua file returns a table; only
//! the keys it sets are applied, so a partial config is fine. Everything above
//! this module consumes the resolved [`Config`] and is agnostic to where a value
//! came from.
//!
//! The applied surface is honest: editor engine/options, theme mode, and UI/mono
//! fonts are all actually wired (see [`Config::apply_theme`] and the editor
//! view). Keybinding remaps are intentionally out of scope here rather than
//! parsed-and-ignored.

use std::path::{Path, PathBuf};

/// Which editing engine backs the editor pane.
///
/// `Native` is gpui-component's built-in code editor (tree-sitter 0.26
/// highlighting, line numbers, folding, LSP-ready). `Helix` drives a custom
/// element from `helix-core`'s rope/selection/transaction model; it is not yet
/// wired (its syntax layer must be aligned to tree-sitter 0.26 first), so the
/// editor view renders an explicit placeholder for it rather than silently
/// falling back to `Native`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EditorEngine {
    #[default]
    Native,
    Helix,
}

impl EditorEngine {
    /// Parse an engine name (case-insensitive). Unknown names fall back to the
    /// default so a typo in config never leaves the editor unusable.
    pub fn parse(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "helix" | "hx" => EditorEngine::Helix,
            _ => EditorEngine::Native,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EditorEngine::Native => "native",
            EditorEngine::Helix => "helix",
        }
    }
}

/// Editor pane configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorConfig {
    pub engine: EditorEngine,
    pub line_number: bool,
    pub indent_guides: bool,
    pub soft_wrap: bool,
    pub tab_size: u8,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            engine: EditorEngine::default(),
            line_number: true,
            indent_guides: true,
            soft_wrap: false,
            tab_size: 4,
        }
    }
}

/// Preferred colour mode. `Glass` layers a macOS window-vibrancy blur behind
/// every pane on top of the dark palette (see [`super::theme`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemePref {
    #[default]
    Dark,
    Light,
    Glass,
}

impl ThemePref {
    pub fn parse(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "light" => ThemePref::Light,
            "glass" | "macos-glass" | "macos_glass" => ThemePref::Glass,
            _ => ThemePref::Dark,
        }
    }

    /// Whether the whole-app macOS vibrancy blur should be enabled.
    pub fn is_glass(self) -> bool {
        matches!(self, ThemePref::Glass)
    }
}

/// Font configuration. `None` fields keep the theme's built-in font.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontConfig {
    pub ui_family: Option<String>,
    pub mono_family: Option<String>,
    pub ui_size: Option<f32>,
    pub mono_size: Option<f32>,
}

/// Agent pane configuration: an explicit command overrides auto-detection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentConfig {
    pub command: Option<String>,
}

/// Top-level resolved configuration.
#[derive(Clone, Debug, Default)]
pub struct Config {
    pub editor: EditorConfig,
    pub theme: ThemePref,
    pub font: FontConfig,
    pub agent: AgentConfig,
}

impl Config {
    /// Resolve config: defaults, then `config.lua`, then env overrides.
    pub fn load() -> Self {
        let mut config = Config::default();
        if let Some(path) = config_file_path() {
            if path.is_file() {
                match std::fs::read_to_string(&path) {
                    Ok(src) => config.apply_lua(&src),
                    Err(e) => eprintln!("config: cannot read {}: {e}", path.display()),
                }
            }
        }
        config.apply_env();
        config
    }

    /// Layer a `config.lua` source over the current values. Parse failures are
    /// reported and ignored (a broken config must not brick startup).
    pub fn apply_lua(&mut self, src: &str) {
        match parse_lua(src) {
            Ok(parsed) => parsed.merge_into(self),
            Err(e) => eprintln!("config: config.lua error: {e}"),
        }
    }

    fn apply_env(&mut self) {
        if let Ok(engine) = std::env::var("WB_EDITOR_ENGINE") {
            self.editor.engine = EditorEngine::parse(&engine);
        }
        if let Ok(mode) = std::env::var("WB_THEME") {
            self.theme = ThemePref::parse(&mode);
        }
    }
}

/// The values a `config.lua` may set. Every field is optional; only present
/// keys override the running config.
#[derive(Default, Debug, PartialEq)]
pub struct LuaConfig {
    pub engine: Option<String>,
    pub line_number: Option<bool>,
    pub indent_guides: Option<bool>,
    pub soft_wrap: Option<bool>,
    pub tab_size: Option<u8>,
    pub theme_mode: Option<String>,
    pub ui_family: Option<String>,
    pub mono_family: Option<String>,
    pub ui_size: Option<f32>,
    pub mono_size: Option<f32>,
    pub agent_command: Option<String>,
}

impl LuaConfig {
    fn merge_into(self, config: &mut Config) {
        if let Some(e) = self.engine {
            config.editor.engine = EditorEngine::parse(&e);
        }
        if let Some(v) = self.line_number {
            config.editor.line_number = v;
        }
        if let Some(v) = self.indent_guides {
            config.editor.indent_guides = v;
        }
        if let Some(v) = self.soft_wrap {
            config.editor.soft_wrap = v;
        }
        if let Some(v) = self.tab_size {
            config.editor.tab_size = v.clamp(1, 16);
        }
        if let Some(m) = self.theme_mode {
            config.theme = ThemePref::parse(&m);
        }
        if self.ui_family.is_some() {
            config.font.ui_family = self.ui_family;
        }
        if self.mono_family.is_some() {
            config.font.mono_family = self.mono_family;
        }
        if self.ui_size.is_some() {
            config.font.ui_size = self.ui_size;
        }
        if self.mono_size.is_some() {
            config.font.mono_size = self.mono_size;
        }
        if self.agent_command.is_some() {
            config.agent.command = self.agent_command;
        }
    }
}

/// Evaluate a `config.lua` source into a [`LuaConfig`]. The script must return a
/// table; nested `editor`, `theme`, and `font` sub-tables are read leniently
/// (missing keys are simply `None`).
pub fn parse_lua(src: &str) -> Result<LuaConfig, String> {
    use mlua::{Lua, Value};
    let lua = Lua::new();
    let value: Value = lua.load(src).eval().map_err(|e| e.to_string())?;
    let Value::Table(root) = value else {
        return Err("config.lua must return a table".to_string());
    };

    let mut out = LuaConfig::default();
    if let Some(editor) = sub(&root, "editor") {
        out.engine = get_str(&editor, "engine");
        out.line_number = get_bool(&editor, "line_number");
        out.indent_guides = get_bool(&editor, "indent_guides");
        out.soft_wrap = get_bool(&editor, "soft_wrap");
        out.tab_size = get_num(&editor, "tab_size").map(|n| n as u8);
    }
    if let Some(theme) = sub(&root, "theme") {
        out.theme_mode = get_str(&theme, "mode");
    }
    if let Some(font) = sub(&root, "font") {
        out.ui_family = get_str(&font, "family");
        out.mono_family = get_str(&font, "mono_family");
        out.ui_size = get_num(&font, "size").map(|n| n as f32);
        out.mono_size = get_num(&font, "mono_size").map(|n| n as f32);
    }
    if let Some(agent) = sub(&root, "agent") {
        out.agent_command = get_str(&agent, "command");
    }
    Ok(out)
}

fn sub<'a>(root: &mlua::Table<'a>, key: &str) -> Option<mlua::Table<'a>> {
    root.get::<_, mlua::Table>(key).ok()
}

// The getters match on the concrete Lua value type rather than relying on
// mlua's coercions: a missing key is Nil, and mlua would coerce Nil to `false`
// for a `bool` target, which would wrongly override a defaulted-true option.
fn get_str(t: &mlua::Table, k: &str) -> Option<String> {
    match t.get::<_, mlua::Value>(k) {
        Ok(mlua::Value::String(s)) => s
            .to_str()
            .ok()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn get_bool(t: &mlua::Table, k: &str) -> Option<bool> {
    match t.get::<_, mlua::Value>(k) {
        Ok(mlua::Value::Boolean(b)) => Some(b),
        _ => None,
    }
}

fn get_num(t: &mlua::Table, k: &str) -> Option<f64> {
    match t.get::<_, mlua::Value>(k) {
        Ok(mlua::Value::Integer(n)) => Some(n as f64),
        Ok(mlua::Value::Number(n)) => Some(n),
        _ => None,
    }
}

/// The config directory: `$LIFEOS_WORKBENCH_CONFIG_DIR` or
/// `~/.config/lifeos-workbench`.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("LIFEOS_WORKBENCH_CONFIG_DIR") {
        return Some(dir.into());
    }
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join(".config/lifeos-workbench"))
}

/// The `config.lua` path inside [`config_dir`].
pub fn config_file_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.lua"))
}

/// Persist one `section.key = value` into `config.lua`, used by the Settings
/// pane and the agent picker. This is a minimal line-based textual upsert
/// (find the section table, replace the key's line or insert one), not a full
/// Lua AST round-trip - untouched keys and the user's own comments survive.
pub fn write_back(section: &str, key: &str, value: &str) -> std::io::Result<()> {
    let Some(path) = config_file_path() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no config directory (HOME unset)",
        ));
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_else(|_| default_skeleton());
    let updated = upsert_kv(&existing, section, key, value);
    std::fs::write(&path, updated)
}

fn default_skeleton() -> String {
    "-- lifeos-workbench config\nreturn {\n}\n".to_string()
}

/// Render a Lua value literal for `value`: booleans/numbers unquoted, anything
/// else as a quoted string.
fn lua_literal(value: &str) -> String {
    if value == "true" || value == "false" || value.parse::<f64>().is_ok() {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// Upsert `section.key = value` into Lua source `src`, preserving everything
/// else. Assumes the flat `return { section = { key = val, ... }, ... }`
/// shape this module always generates/reads - not a general Lua formatter.
fn upsert_kv(src: &str, section: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = src.lines().map(|l| l.to_string()).collect();
    let new_line = format!("        {key} = {},", lua_literal(value));

    let section_start = lines.iter().position(|l| {
        let t = l.trim();
        t.starts_with(&format!("{section} = {{")) || t.starts_with(&format!("{section}={{"))
    });

    match section_start {
        Some(start) => {
            let mut depth = 0i32;
            let mut end = start;
            for (i, l) in lines.iter().enumerate().skip(start) {
                depth += l.matches('{').count() as i32;
                depth -= l.matches('}').count() as i32;
                if depth == 0 && i > start {
                    end = i;
                    break;
                }
            }
            let key_line = lines[(start + 1)..end].iter().position(|l| {
                let t = l.trim_start();
                t.starts_with(&format!("{key} ")) || t.starts_with(&format!("{key}="))
            });
            match key_line {
                Some(offset) => lines[start + 1 + offset] = new_line,
                None => lines.insert(start + 1, new_line),
            }
        }
        None => {
            let insert_at = lines
                .iter()
                .rposition(|l| l.trim() == "}")
                .unwrap_or(lines.len());
            for (offset, l) in [
                format!("    {section} = {{"),
                new_line,
                "    },".to_string(),
            ]
            .into_iter()
            .enumerate()
            {
                lines.insert(insert_at + offset, l);
            }
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_defaults_to_native() {
        assert_eq!(EditorEngine::default(), EditorEngine::Native);
        assert_eq!(EditorConfig::default().engine, EditorEngine::Native);
    }

    #[test]
    fn parses_helix_case_insensitively() {
        assert_eq!(EditorEngine::parse("helix"), EditorEngine::Helix);
        assert_eq!(EditorEngine::parse("  Helix "), EditorEngine::Helix);
        assert_eq!(EditorEngine::parse("HX"), EditorEngine::Helix);
    }

    #[test]
    fn unknown_engine_falls_back_to_native() {
        assert_eq!(EditorEngine::parse("vim"), EditorEngine::Native);
        assert_eq!(EditorEngine::parse(""), EditorEngine::Native);
    }

    #[test]
    fn lua_config_overrides_only_present_keys() {
        let src = r#"
            return {
                editor = { engine = "helix", tab_size = 2, soft_wrap = true },
                theme = { mode = "light" },
                font = { mono_family = "JetBrains Mono", mono_size = 13 },
            }
        "#;
        let mut config = Config::default();
        config.apply_lua(src);
        assert_eq!(config.editor.engine, EditorEngine::Helix);
        assert_eq!(config.editor.tab_size, 2);
        assert!(config.editor.soft_wrap);
        // untouched keys keep their defaults
        assert!(config.editor.line_number);
        assert_eq!(config.theme, ThemePref::Light);
        assert_eq!(config.font.mono_family.as_deref(), Some("JetBrains Mono"));
        assert_eq!(config.font.mono_size, Some(13.0));
        assert_eq!(config.font.ui_family, None);
    }

    #[test]
    fn tab_size_is_clamped_to_a_sane_range() {
        let mut config = Config::default();
        config.apply_lua("return { editor = { tab_size = 99 } }");
        assert_eq!(config.editor.tab_size, 16);
        config.apply_lua("return { editor = { tab_size = 0 } }");
        assert_eq!(config.editor.tab_size, 1);
    }

    #[test]
    fn a_non_table_return_is_an_error_not_a_panic() {
        assert!(parse_lua("return 42").is_err());
        // A broken config leaves defaults intact.
        let mut config = Config::default();
        config.apply_lua("return 42");
        assert_eq!(config.editor.engine, EditorEngine::Native);
    }

    #[test]
    fn empty_or_bare_table_yields_all_defaults() {
        let parsed = parse_lua("return {}").unwrap();
        assert_eq!(parsed, LuaConfig::default());
    }

    #[test]
    fn glass_theme_parses_from_either_spelling() {
        assert_eq!(ThemePref::parse("glass"), ThemePref::Glass);
        assert_eq!(ThemePref::parse("macos-glass"), ThemePref::Glass);
        assert!(ThemePref::Glass.is_glass());
        assert!(!ThemePref::Dark.is_glass());
    }

    #[test]
    fn agent_command_parses_from_lua() {
        let parsed = parse_lua(r#"return { agent = { command = "codex" } }"#).unwrap();
        assert_eq!(parsed.agent_command.as_deref(), Some("codex"));
        let mut config = Config::default();
        config.apply_lua(r#"return { agent = { command = "codex" } }"#);
        assert_eq!(config.agent.command.as_deref(), Some("codex"));
    }

    #[test]
    fn write_back_inserts_a_new_section_into_a_bare_skeleton() {
        let src = default_skeleton();
        let updated = upsert_kv(&src, "agent", "command", "claude");
        let parsed = parse_lua(&updated).expect("round-trips through parse_lua");
        assert_eq!(parsed.agent_command.as_deref(), Some("claude"));
    }

    #[test]
    fn write_back_replaces_an_existing_key_without_disturbing_others() {
        let src = r#"return {
    editor = {
        engine = "helix",
    },
    agent = {
        command = "claude",
    },
}
"#;
        let updated = upsert_kv(src, "agent", "command", "codex");
        let parsed = parse_lua(&updated).expect("round-trips");
        assert_eq!(parsed.agent_command.as_deref(), Some("codex"));
        assert_eq!(
            parsed.engine.as_deref(),
            Some("helix"),
            "untouched key survives"
        );
    }

    #[test]
    fn write_back_bool_and_number_values_stay_unquoted() {
        let updated = upsert_kv(&default_skeleton(), "editor", "soft_wrap", "true");
        let parsed = parse_lua(&updated).unwrap();
        assert_eq!(parsed.soft_wrap, Some(true));
        let updated = upsert_kv(&default_skeleton(), "font", "size", "14");
        let parsed = parse_lua(&updated).unwrap();
        assert_eq!(parsed.ui_size, Some(14.0));
    }
}
