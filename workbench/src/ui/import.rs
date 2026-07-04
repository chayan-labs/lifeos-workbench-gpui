//! First-launch config import from VS Code and WezTerm.
//!
//! On the very first launch (no `config.lua` yet) the Workbench looks for a VS
//! Code `settings.json` and a WezTerm `wezterm.lua` and, if found, seeds a
//! `config.lua` from them so the app opens feeling like the user's existing
//! tools. VS Code's JSON (with comments) is parsed as JSON5; WezTerm's config is
//! genuinely evaluated with `mlua` (a small `wezterm` stub stands in for the
//! module it requires). Only the handful of keys we actually map are read, and
//! the generated file is a plain, user-editable `config.lua` with a provenance
//! header - nothing is hidden or applied behind the user's back.

use std::path::{Path, PathBuf};

/// What a first-launch import produced.
#[derive(Debug, Default, PartialEq)]
pub struct Imported {
    pub tab_size: Option<u8>,
    pub theme_mode: Option<String>,
    pub mono_family: Option<String>,
    pub mono_size: Option<f32>,
    /// Human-readable provenance (e.g. "VS Code", "WezTerm").
    pub sources: Vec<String>,
}

impl Imported {
    fn is_empty(&self) -> bool {
        self.tab_size.is_none()
            && self.theme_mode.is_none()
            && self.mono_family.is_none()
            && self.mono_size.is_none()
    }

    /// Render a `config.lua` the user can read and edit.
    pub fn to_config_lua(&self) -> String {
        let mut out = String::new();
        out.push_str("-- Life OS Workbench config.\n");
        if self.sources.is_empty() {
            out.push_str("-- Generated with defaults on first launch.\n");
        } else {
            out.push_str(&format!(
                "-- Imported on first launch from: {}.\n",
                self.sources.join(", ")
            ));
        }
        out.push_str("-- Edit freely; this file is never overwritten.\n\nreturn {\n");

        out.push_str("  editor = {\n");
        out.push_str("    engine = \"native\",\n");
        if let Some(t) = self.tab_size {
            out.push_str(&format!("    tab_size = {t},\n"));
        }
        out.push_str("  },\n");

        if let Some(mode) = &self.theme_mode {
            out.push_str(&format!("  theme = {{ mode = \"{mode}\" }},\n"));
        }

        if self.mono_family.is_some() || self.mono_size.is_some() {
            out.push_str("  font = {\n");
            if let Some(fam) = &self.mono_family {
                out.push_str(&format!("    mono_family = \"{}\",\n", lua_escape(fam)));
            }
            if let Some(size) = self.mono_size {
                out.push_str(&format!("    mono_size = {size},\n"));
            }
            out.push_str("  },\n");
        }

        out.push_str("}\n");
        out
    }
}

/// Run the first-launch import if no `config.lua` exists yet. Returns the path
/// written, or `None` if a config already exists or the directory is unknown.
pub fn run_first_launch_import(config_dir: &Path) -> Option<PathBuf> {
    let config_path = config_dir.join("config.lua");
    if config_path.exists() {
        return None; // already configured; never overwrite
    }

    let mut imported = Imported::default();
    if let Some(vscode) = vscode_settings_path() {
        import_vscode(&vscode, &mut imported);
    }
    if let Some(wez) = wezterm_config_path() {
        import_wezterm(&wez, &mut imported);
    }

    if std::fs::create_dir_all(config_dir).is_err() {
        return None;
    }
    let contents = imported.to_config_lua();
    match std::fs::write(&config_path, contents) {
        Ok(()) => {
            if imported.is_empty() {
                eprintln!("config: wrote default {}", config_path.display());
            } else {
                eprintln!(
                    "config: imported {} into {}",
                    imported.sources.join(" + "),
                    config_path.display()
                );
            }
            Some(config_path)
        }
        Err(e) => {
            eprintln!("config: could not write {}: {e}", config_path.display());
            None
        }
    }
}

fn vscode_settings_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let p = Path::new(&home).join("Library/Application Support/Code/User/settings.json");
    p.is_file().then_some(p)
}

fn wezterm_config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let home = Path::new(&home);
    [
        home.join(".wezterm.lua"),
        home.join(".config/wezterm/wezterm.lua"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Parse VS Code `settings.json` (JSON with comments -> JSON5) for the keys we
/// map: tab size, editor font, and a dark/light heuristic from the theme name.
pub fn import_vscode(path: &Path, out: &mut Imported) {
    let Ok(src) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(value) = json5::from_str::<serde_json::Value>(&src) else {
        return;
    };
    let mut touched = false;
    if let Some(n) = value.get("editor.tabSize").and_then(|v| v.as_u64()) {
        out.tab_size = Some(n.clamp(1, 16) as u8);
        touched = true;
    }
    if let Some(f) = value.get("editor.fontFamily").and_then(|v| v.as_str()) {
        if let Some(first) = first_font_family(f) {
            out.mono_family = Some(first);
            touched = true;
        }
    }
    if let Some(s) = value.get("editor.fontSize").and_then(|v| v.as_f64()) {
        out.mono_size = Some(s as f32);
        touched = true;
    }
    if let Some(theme) = value.get("workbench.colorTheme").and_then(|v| v.as_str()) {
        out.theme_mode = Some(if theme.to_ascii_lowercase().contains("light") {
            "light".to_string()
        } else {
            "dark".to_string()
        });
        touched = true;
    }
    if touched {
        out.sources.push("VS Code".to_string());
    }
}

/// Evaluate a WezTerm `wezterm.lua` with `mlua`, standing in a small `wezterm`
/// stub for the module it requires, and read the font family + size off the
/// returned config table. A config that errors under the stub is skipped.
pub fn import_wezterm(path: &Path, out: &mut Imported) {
    let Ok(src) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok((family, size)) = eval_wezterm(&src) else {
        return;
    };
    let mut touched = false;
    if let Some(fam) = family {
        // Prefer WezTerm's mono font if VS Code did not already supply one.
        if out.mono_family.is_none() {
            out.mono_family = Some(fam);
            touched = true;
        }
    }
    if let Some(s) = size {
        if out.mono_size.is_none() {
            out.mono_size = Some(s);
            touched = true;
        }
    }
    if touched {
        out.sources.push("WezTerm".to_string());
    }
}

fn eval_wezterm(src: &str) -> Result<(Option<String>, Option<f32>), String> {
    use mlua::{Lua, Table, Value};
    let lua = Lua::new();

    // A minimal `wezterm` module: enough for the common `require 'wezterm'`
    // + `config_builder()` + `font()/font_with_fallback()` shape.
    let wez = lua.create_table().map_err(|e| e.to_string())?;
    let font_fn = lua
        .create_function(|lua, (name, _opts): (String, Option<Table>)| {
            let t = lua.create_table()?;
            t.set("family", name)?;
            Ok(t)
        })
        .map_err(|e| e.to_string())?;
    wez.set("font", font_fn).map_err(|e| e.to_string())?;
    let fallback_fn = lua
        .create_function(|lua, (list,): (Table,)| {
            let first: Value = list.get(1).unwrap_or(Value::Nil);
            let family = match first {
                Value::String(s) => s.to_str().map(|s| s.to_string()).unwrap_or_default(),
                Value::Table(t) => t.get::<_, String>("family").unwrap_or_default(),
                _ => String::new(),
            };
            let t = lua.create_table()?;
            t.set("family", family)?;
            Ok(t)
        })
        .map_err(|e| e.to_string())?;
    wez.set("font_with_fallback", fallback_fn)
        .map_err(|e| e.to_string())?;
    let builder_fn = lua
        .create_function(|lua, ()| lua.create_table())
        .map_err(|e| e.to_string())?;
    wez.set("config_builder", builder_fn)
        .map_err(|e| e.to_string())?;
    // Absorb unknown accesses (e.g. wezterm.log_info) with a no-op metatable.
    let meta = lua.create_table().map_err(|e| e.to_string())?;
    let index_fn = lua
        .create_function(|lua, (_t, _k): (Table, Value)| {
            lua.create_function(|_, _: mlua::MultiValue| Ok(()))
        })
        .map_err(|e| e.to_string())?;
    meta.set("__index", index_fn).map_err(|e| e.to_string())?;
    wez.set_metatable(Some(meta));

    lua.globals()
        .get::<_, Table>("package")
        .and_then(|p| p.get::<_, Table>("loaded"))
        .and_then(|l| l.set("wezterm", wez))
        .map_err(|e| e.to_string())?;

    let value: Value = lua.load(src).eval().map_err(|e| e.to_string())?;
    let Value::Table(config) = value else {
        return Ok((None, None));
    };
    let family = config
        .get::<_, Table>("font")
        .ok()
        .and_then(|f| f.get::<_, String>("family").ok())
        .filter(|s| !s.is_empty());
    let size = config.get::<_, f64>("font_size").ok().map(|n| n as f32);
    Ok((family, size))
}

/// Take the first family from a CSS-style font list (`"A", "B", monospace`).
fn first_font_family(list: &str) -> Option<String> {
    list.split(',')
        .next()
        .map(|s| s.trim().trim_matches(['"', '\'']).to_string())
        .filter(|s| !s.is_empty() && s != "monospace")
}

fn lua_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vscode_settings_map_to_imported_fields() {
        let dir = std::env::temp_dir().join("wb-import-vscode-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        std::fs::write(
            &path,
            r#"{
                // a comment, JSON5-style
                "editor.tabSize": 2,
                "editor.fontFamily": "'JetBrains Mono', Menlo, monospace",
                "editor.fontSize": 13,
                "workbench.colorTheme": "Solarized Light",
            }"#,
        )
        .unwrap();
        let mut imported = Imported::default();
        import_vscode(&path, &mut imported);
        assert_eq!(imported.tab_size, Some(2));
        assert_eq!(imported.mono_family.as_deref(), Some("JetBrains Mono"));
        assert_eq!(imported.mono_size, Some(13.0));
        assert_eq!(imported.theme_mode.as_deref(), Some("light"));
        assert_eq!(imported.sources, vec!["VS Code".to_string()]);
    }

    #[test]
    fn wezterm_lua_is_evaluated_for_font() {
        let src = r#"
            local wezterm = require 'wezterm'
            local config = wezterm.config_builder()
            config.font = wezterm.font('Fira Code')
            config.font_size = 14
            return config
        "#;
        let (family, size) = eval_wezterm(src).unwrap();
        assert_eq!(family.as_deref(), Some("Fira Code"));
        assert_eq!(size, Some(14.0));
    }

    #[test]
    fn wezterm_font_with_fallback_takes_the_first() {
        let src = r#"
            local wezterm = require 'wezterm'
            return { font = wezterm.font_with_fallback({ 'Cascadia Code', 'Menlo' }) }
        "#;
        let (family, _) = eval_wezterm(src).unwrap();
        assert_eq!(family.as_deref(), Some("Cascadia Code"));
    }

    #[test]
    fn generated_config_lua_reparses_to_the_same_values() {
        let imported = Imported {
            tab_size: Some(2),
            theme_mode: Some("light".to_string()),
            mono_family: Some("JetBrains Mono".to_string()),
            mono_size: Some(13.0),
            sources: vec!["VS Code".to_string()],
        };
        let lua = imported.to_config_lua();
        let parsed = crate::ui::config::parse_lua(&lua).expect("generated config parses");
        assert_eq!(parsed.tab_size, Some(2));
        assert_eq!(parsed.theme_mode.as_deref(), Some("light"));
        assert_eq!(parsed.mono_family.as_deref(), Some("JetBrains Mono"));
        assert_eq!(parsed.mono_size, Some(13.0));
    }

    #[test]
    fn empty_import_writes_a_valid_default_config() {
        let imported = Imported::default();
        let lua = imported.to_config_lua();
        assert!(crate::ui::config::parse_lua(&lua).is_ok());
    }
}
