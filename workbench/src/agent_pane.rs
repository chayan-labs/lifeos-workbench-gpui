//! The agent pane: a conversation transcript over an `AcpAgent`, an input
//! line, and the inline diff-review UI for staged edits (accept/reject per
//! hunk). Resource-state owned by the `PaneStore`, like terminals/editors.

use crate::acp::{AcpAgent, Entry};
use crate::theme::Theme;
use crossterm::event::KeyCode;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::path::Path;

/// Which part of the pane receives keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Input,
    Review { hunk: usize },
}

pub struct AgentPane {
    pub agent: Option<AcpAgent>,
    input: String,
    focus: Focus,
}

/// The agent command: `WORKBENCH_AGENT_CMD` or the Claude Code ACP adapter.
pub fn agent_command() -> String {
    std::env::var("WORKBENCH_AGENT_CMD").unwrap_or_else(|_| "claude-code-acp".into())
}

/// The toolbelt passed at session/new: this same binary re-entered as a
/// stdio MCP server over the in-process lifeos-api (issue #17).
pub fn toolbelt_servers() -> Vec<serde_json::Value> {
    let Ok(exe) = std::env::current_exe() else {
        return Vec::new();
    };
    vec![serde_json::json!({
        "name": "lifeos",
        "command": exe.display().to_string(),
        "args": ["--mcp"],
        "env": []
    })]
}

impl AgentPane {
    pub fn spawn(cwd: &Path) -> Self {
        Self {
            agent: AcpAgent::spawn(&agent_command(), cwd, toolbelt_servers()),
            input: String::new(),
            focus: Focus::Input,
        }
    }

    fn has_edits(&self) -> bool {
        self.agent
            .as_ref()
            .and_then(|a| a.conversation.lock().ok().map(|c| !c.edits.is_empty()))
            .unwrap_or(false)
    }

    /// Feed one key; returns true when consumed.
    pub fn on_key(&mut self, code: KeyCode) -> bool {
        match self.focus {
            Focus::Input => self.on_key_input(code),
            Focus::Review { hunk } => self.on_key_review(code, hunk),
        }
    }

    fn on_key_input(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Tab if self.has_edits() => self.focus = Focus::Review { hunk: 0 },
            KeyCode::Enter => {
                let text = self.input.trim().to_string();
                if !text.is_empty() {
                    if let Some(agent) = &self.agent {
                        agent.prompt(&text);
                    }
                    self.input.clear();
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => return false,
        }
        true
    }

    fn on_key_review(&mut self, code: KeyCode, hunk: usize) -> bool {
        let Some(agent) = &self.agent else {
            self.focus = Focus::Input;
            return true;
        };
        let hunk_count = agent
            .conversation
            .lock()
            .ok()
            .and_then(|c| c.edits.first().map(|e| e.hunks.len()))
            .unwrap_or(0);
        match code {
            KeyCode::Esc | KeyCode::Tab => self.focus = Focus::Input,
            KeyCode::Up | KeyCode::Char('k') => {
                self.focus = Focus::Review {
                    hunk: hunk.saturating_sub(1),
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.focus = Focus::Review {
                    hunk: (hunk + 1).min(hunk_count.saturating_sub(1)),
                }
            }
            KeyCode::Char(' ') => agent.toggle_hunk(0, hunk),
            KeyCode::Char('a') => {
                agent.resolve_edit(0, true);
                self.focus = Focus::Input;
            }
            KeyCode::Char('r') => {
                agent.resolve_edit(0, false);
                self.focus = Focus::Input;
            }
            _ => return false,
        }
        true
    }

    /// Render the transcript, review block, and input line.
    pub fn render_lines(&self, theme: &Theme, height: usize) -> Vec<Line<'static>> {
        let mut lines: Vec<Line> = Vec::new();
        let Some(agent) = &self.agent else {
            return vec![
                Line::styled("agent unavailable".to_string(), theme.gated()),
                Line::styled(
                    format!(
                        "could not start `{}` (set WORKBENCH_AGENT_CMD)",
                        agent_command()
                    ),
                    theme.muted(),
                ),
            ];
        };
        let Ok(c) = agent.conversation.lock() else {
            return lines;
        };
        for entry in &c.entries {
            lines.extend(match entry {
                Entry::User(t) => vec![Line::styled(format!("▸ {t}"), theme.title())],
                Entry::Agent(t) => t
                    .lines()
                    .map(|l| Line::styled(l.to_string(), theme.text()))
                    .collect(),
                Entry::ToolCall(t) => vec![Line::styled(format!("⚙ {t}"), theme.muted())],
                Entry::Info(t) => vec![Line::styled(format!("· {t}"), theme.muted())],
            });
        }
        if let Some(edit) = c.edits.first() {
            let review = matches!(self.focus, Focus::Review { .. });
            let cursor = match self.focus {
                Focus::Review { hunk } => hunk,
                Focus::Input => usize::MAX,
            };
            lines.push(Line::styled(
                format!(
                    "── review {} (space toggle · a accept · r reject{})",
                    edit.path.display(),
                    if review { "" } else { " · tab to focus" }
                ),
                theme.gated(),
            ));
            for (i, hunk) in edit.hunks.iter().enumerate() {
                let on = edit.accepted.contains(&i);
                let marker = if on { "[x]" } else { "[ ]" };
                let style = if review && i == cursor {
                    theme.active_item()
                } else {
                    theme.text()
                };
                lines.push(Line::styled(
                    format!("{marker} hunk {} @ line {}", i + 1, hunk.old_start + 1),
                    style,
                ));
                for old in &hunk.old {
                    lines.push(Line::styled(
                        format!("  - {old}"),
                        Style::default().fg(Color::Indexed(203)),
                    ));
                }
                for new in &hunk.new {
                    lines.push(Line::styled(
                        format!("  + {new}"),
                        Style::default().fg(Color::Indexed(48)),
                    ));
                }
            }
        }
        let prompt = if c.busy { "… " } else { "▸ " };
        drop(c);
        // Keep the tail visible: transcript scrolls, input is the last row.
        let body_rows = height.saturating_sub(1);
        if lines.len() > body_rows {
            lines.drain(..lines.len() - body_rows);
        }
        lines.push(Line::from(vec![
            Span::styled(prompt.to_string(), theme.title()),
            Span::styled(self.input.clone(), theme.text()),
            Span::styled(
                " ".to_string(),
                Style::default().add_modifier(Modifier::REVERSED),
            ),
        ]));
        lines
    }

    /// Statusline fragment for the agent segment.
    pub fn status(&self) -> String {
        match &self.agent {
            None => "agent: off".into(),
            Some(a) => match a.conversation.lock() {
                Ok(c) if c.busy => "agent: busy".into(),
                Ok(c) if !c.edits.is_empty() => format!("agent: {} edits pending", c.edits.len()),
                _ => "agent: idle".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ColorSupport;

    #[test]
    fn typing_and_enter_builds_the_input_line() {
        let mut pane = AgentPane {
            agent: None,
            input: String::new(),
            focus: Focus::Input,
        };
        for c in "hi".chars() {
            pane.on_key(KeyCode::Char(c));
        }
        assert_eq!(pane.input, "hi");
        pane.on_key(KeyCode::Backspace);
        assert_eq!(pane.input, "h");
        // Enter with no agent clears the input without panicking.
        pane.on_key(KeyCode::Enter);
        assert_eq!(pane.input, "");
    }

    #[test]
    fn renders_the_unavailable_state_honestly() {
        let pane = AgentPane {
            agent: None,
            input: String::new(),
            focus: Focus::Input,
        };
        let theme = Theme::new(ColorSupport::TrueColor);
        let lines = pane.render_lines(&theme, 10);
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("agent unavailable"));
        assert_eq!(pane.status(), "agent: off");
    }
}
