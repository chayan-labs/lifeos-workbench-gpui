//! Styled markdown for detail bodies (learning articles, notes, agent
//! output): headings, emphasis, inline/backtick code, bullets, and code
//! fences - the terminal-legible subset, not a full CommonMark engine.

use crate::theme::{Theme, ACCENT, PRIMARY, SUCCESS};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

pub fn render(source: &str, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_fence = false;
    for raw in source.lines() {
        if raw.trim_start().starts_with("```") {
            in_fence = !in_fence;
            lines.push(Line::styled(raw.to_string(), theme.muted()));
            continue;
        }
        if in_fence {
            lines.push(Line::styled(
                raw.to_string(),
                Style::default().fg(SUCCESS.resolve(theme.support)),
            ));
            continue;
        }
        lines.push(inline_line(raw, theme));
    }
    lines
}

fn inline_line(raw: &str, theme: &Theme) -> Line<'static> {
    if let Some(rest) = raw.strip_prefix("### ") {
        return Line::styled(rest.to_string(), theme.title());
    }
    if let Some(rest) = raw.strip_prefix("## ") {
        return Line::styled(
            rest.to_string(),
            Style::default()
                .fg(PRIMARY.resolve(theme.support))
                .add_modifier(Modifier::BOLD),
        );
    }
    if let Some(rest) = raw.strip_prefix("# ") {
        return Line::styled(
            rest.to_string(),
            Style::default()
                .fg(ACCENT.resolve(theme.support))
                .add_modifier(Modifier::BOLD),
        );
    }
    let (indent, text) = match raw.trim_start().strip_prefix("- ") {
        Some(item) => (
            format!("{}• ", &raw[..raw.len() - raw.trim_start().len()]),
            item,
        ),
        None => (String::new(), raw),
    };
    let mut spans = vec![Span::styled(indent, theme.text())];
    spans.extend(inline_spans(text, theme));
    Line::from(spans)
}

/// Split `**bold**`, `*italic*`, and `` `code` `` runs into styled spans.
fn inline_spans(text: &str, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        // At an equal index the longer delimiter wins, so "**" beats "*".
        let next = ["**", "`", "*"]
            .iter()
            .filter_map(|d| rest.find(*d).map(|i| (i, *d)))
            .min_by(|a, b| a.0.cmp(&b.0).then(b.1.len().cmp(&a.1.len())));
        let Some((start, delim)) = next else {
            spans.push(Span::styled(rest.to_string(), theme.text()));
            break;
        };
        if start > 0 {
            spans.push(Span::styled(rest[..start].to_string(), theme.text()));
        }
        let after = &rest[start + delim.len()..];
        match after.find(delim) {
            Some(end) => {
                let inner = after[..end].to_string();
                let style = match delim {
                    "**" => theme.text().add_modifier(Modifier::BOLD),
                    "*" => theme.text().add_modifier(Modifier::ITALIC),
                    _ => Style::default().fg(SUCCESS.resolve(theme.support)),
                };
                spans.push(Span::styled(inner, style));
                rest = &after[end + delim.len()..];
            }
            None => {
                spans.push(Span::styled(rest[start..].to_string(), theme.text()));
                break;
            }
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ColorSupport;

    fn text_of(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn headings_bullets_and_emphasis_render() {
        let theme = Theme::new(ColorSupport::TrueColor);
        let lines = render(
            "# Title\n- item with **bold** and `code`\nplain *it*",
            &theme,
        );
        assert_eq!(text_of(&lines[0]), "Title");
        assert_eq!(text_of(&lines[1]), "• item with bold and code");
        let bold = &lines[1].spans.iter().find(|s| s.content == "bold").unwrap();
        assert!(bold.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(text_of(&lines[2]), "plain it");
    }

    #[test]
    fn code_fences_pass_through_unstyled_content() {
        let theme = Theme::new(ColorSupport::TrueColor);
        let lines = render("```\nlet **x** = 1;\n```", &theme);
        assert_eq!(text_of(&lines[1]), "let **x** = 1;");
    }

    #[test]
    fn unclosed_delimiter_is_kept_literally() {
        let theme = Theme::new(ColorSupport::TrueColor);
        let lines = render("a **dangling", &theme);
        assert_eq!(text_of(&lines[0]), "a **dangling");
    }
}
