//! File-extension to language-name mapping.
//!
//! The names returned are gpui-component highlighter language identifiers (or
//! their short aliases), matched against the `tree-sitter-*` grammars enabled
//! in `Cargo.toml`. An unknown extension maps to `"text"` (no highlighting),
//! which the highlighter always accepts.

use std::path::Path;

/// The plain-text fallback language: always registered, never highlighted.
pub const PLAIN: &str = "text";

/// Map a path's extension (or well-known filename) to a highlighter language.
pub fn language_for(path: &Path) -> &'static str {
    // A few languages are keyed by full filename, not extension.
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        match name {
            "Cargo.lock" => return "toml",
            "Makefile" | "makefile" => return "make",
            "Dockerfile" => return "bash",
            _ => {}
        }
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "rs" => "rust",
        "py" | "pyi" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "json" | "json5" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "sh" | "bash" | "zsh" => "bash",
        "go" => "go",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => "cpp",
        "html" | "htm" => "html",
        "css" => "css",
        "lua" => "lua",
        "sql" => "sql",
        _ => PLAIN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn lang(p: &str) -> &'static str {
        language_for(&PathBuf::from(p))
    }

    #[test]
    fn maps_common_source_extensions() {
        assert_eq!(lang("src/main.rs"), "rust");
        assert_eq!(lang("app.py"), "python");
        assert_eq!(lang("index.ts"), "typescript");
        assert_eq!(lang("view.tsx"), "tsx");
        assert_eq!(lang("data.json"), "json");
        assert_eq!(lang("Cargo.toml"), "toml");
    }

    #[test]
    fn maps_well_known_filenames() {
        assert_eq!(lang("Cargo.lock"), "toml");
        assert_eq!(lang("Makefile"), "make");
        assert_eq!(lang("Dockerfile"), "bash");
    }

    #[test]
    fn unknown_extension_is_plain_text() {
        assert_eq!(lang("notes.xyz"), PLAIN);
        assert_eq!(lang("no_extension"), PLAIN);
    }

    #[test]
    fn extension_match_is_case_insensitive() {
        assert_eq!(lang("MAIN.RS"), "rust");
        assert_eq!(lang("README.MD"), "markdown");
    }
}
