//! Runtime configuration consumed by the gpui views.
//!
//! For now this is a small, defaulted struct with an environment override so
//! the editor engine and its options are selectable before the full Lua
//! (`mlua`) config loader lands in step #26. The Lua loader will populate the
//! same struct, so nothing above this module changes when it arrives.

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

/// Top-level config. Extended by the Lua loader (theme/font/keymap) in #26.
#[derive(Clone, Debug, Default)]
pub struct Config {
    pub editor: EditorConfig,
}

impl Config {
    /// Load configuration. Until the Lua loader lands this reads a single
    /// environment override (`WB_EDITOR_ENGINE=helix|native`) so the engine
    /// switch is testable end-to-end; everything else takes defaults.
    pub fn load() -> Self {
        let mut config = Config::default();
        if let Ok(engine) = std::env::var("WB_EDITOR_ENGINE") {
            config.editor.engine = EditorEngine::parse(&engine);
        }
        config
    }
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
}
