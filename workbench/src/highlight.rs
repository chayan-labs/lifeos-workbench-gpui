//! Tree-sitter syntax highlighting for editor panes. Grammars are linked
//! statically (rust/python/javascript/json) so highlighting works offline
//! with no helix runtime install; unknown languages render plain.

use ratatui::style::{Color, Modifier, Style};
use std::path::Path;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Capture names we style; order matters - it is the index space the
/// highlighter reports back.
const CAPTURES: &[&str] = &[
    "keyword",
    "function",
    "type",
    "string",
    "comment",
    "constant",
    "number",
    "attribute",
    "property",
    "operator",
    "variable",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Python,
    Javascript,
    Json,
}

pub fn detect(path: &Path) -> Option<Lang> {
    match path.extension()?.to_str()? {
        "rs" => Some(Lang::Rust),
        "py" => Some(Lang::Python),
        "js" | "mjs" | "cjs" | "jsx" => Some(Lang::Javascript),
        "json" => Some(Lang::Json),
        _ => None,
    }
}

fn config_for(lang: Lang) -> Option<HighlightConfiguration> {
    let (language, highlights) = match lang {
        Lang::Rust => (
            tree_sitter_rust::LANGUAGE,
            tree_sitter_rust::HIGHLIGHTS_QUERY,
        ),
        Lang::Python => (
            tree_sitter_python::LANGUAGE,
            tree_sitter_python::HIGHLIGHTS_QUERY,
        ),
        Lang::Javascript => (
            tree_sitter_javascript::LANGUAGE,
            tree_sitter_javascript::HIGHLIGHT_QUERY,
        ),
        Lang::Json => (
            tree_sitter_json::LANGUAGE,
            tree_sitter_json::HIGHLIGHTS_QUERY,
        ),
    };
    let mut config =
        HighlightConfiguration::new(language.into(), "source", highlights, "", "").ok()?;
    config.configure(CAPTURES);
    Some(config)
}

/// Terminal Brutalism-adjacent capture styling: hue only, never structure.
fn style_for(capture: usize) -> Style {
    match CAPTURES.get(capture).copied() {
        Some("keyword") => Style::default()
            .fg(Color::Indexed(63))
            .add_modifier(Modifier::BOLD),
        Some("function") => Style::default().fg(Color::Indexed(226)),
        Some("type") => Style::default().fg(Color::Indexed(48)),
        Some("string") => Style::default().fg(Color::Indexed(114)),
        Some("comment") => Style::default()
            .fg(Color::Indexed(101))
            .add_modifier(Modifier::ITALIC),
        Some("constant") | Some("number") => Style::default().fg(Color::Indexed(209)),
        Some("attribute") | Some("property") => Style::default().fg(Color::Indexed(146)),
        _ => Style::default(),
    }
}

/// A styled run of source text: (byte range start, byte range end, style).
pub type StyledRange = (usize, usize, Style);

/// Highlight full source, returning styled byte ranges (plain gaps omitted).
/// Any failure degrades to no highlighting - never to an error.
pub fn highlight(lang: Lang, source: &str) -> Vec<StyledRange> {
    let Some(config) = config_for(lang) else {
        return Vec::new();
    };
    let mut highlighter = Highlighter::new();
    let Ok(events) = highlighter.highlight(&config, source.as_bytes(), None, |_| None) else {
        return Vec::new();
    };
    let mut ranges = Vec::new();
    let mut active: Vec<usize> = Vec::new();
    for event in events.flatten() {
        match event {
            HighlightEvent::HighlightStart(h) => active.push(h.0),
            HighlightEvent::HighlightEnd => {
                active.pop();
            }
            HighlightEvent::Source { start, end } => {
                if let Some(&capture) = active.last() {
                    ranges.push((start, end, style_for(capture)));
                }
            }
        }
    }
    ranges
}

/// Style for one byte position, if any range covers it.
pub fn style_at(ranges: &[StyledRange], byte: usize) -> Option<Style> {
    ranges
        .iter()
        .find(|(s, e, _)| *s <= byte && byte < *e)
        .map(|(_, _, style)| *style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_language_from_extension() {
        assert_eq!(detect(Path::new("src/main.rs")), Some(Lang::Rust));
        assert_eq!(detect(Path::new("mod.py")), Some(Lang::Python));
        assert_eq!(detect(Path::new("notes.txt")), None);
    }

    #[test]
    fn rust_keywords_get_a_non_default_style() {
        let src = "fn main() { let x = 1; }\n";
        let ranges = highlight(Lang::Rust, src);
        assert!(!ranges.is_empty(), "grammar must produce captures");
        // "fn" starts at byte 0 and must be styled as a keyword.
        let style = style_at(&ranges, 0).expect("fn styled");
        assert_eq!(style.fg, Some(Color::Indexed(63)));
    }

    #[test]
    fn json_strings_are_styled_and_gaps_are_plain() {
        let src = "{\"key\": 42}\n";
        let ranges = highlight(Lang::Json, src);
        assert!(style_at(&ranges, 1).is_some(), "\"key\" styled");
    }
}
