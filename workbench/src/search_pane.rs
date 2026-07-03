//! Search pane (issue #14): hybrid recall over the derived DB, in-process.
//! The query goes to `/api/search` (FTS5 lexical + memvec semantic, RRF
//! fused, lexical-only degradation) via the same `InProcessApi` handle the
//! rest of the app uses - no socket, no second process.

use crate::api::InProcessApi;
use crate::theme::Theme;
use crossterm::event::KeyCode;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// One fused hit, reduced to what the pane renders.
#[derive(Clone, Debug)]
pub struct Hit {
    pub title: String,
    pub module: String,
    pub entity_type: String,
    pub id: String,
}

#[derive(Clone, Debug, Default)]
pub struct SearchResults {
    pub hits: Vec<Hit>,
    pub mode: String,
    pub busy: bool,
}

pub struct SearchPane {
    api: InProcessApi,
    token: Option<String>,
    pub query: String,
    pub selected: usize,
    pub results: Arc<Mutex<SearchResults>>,
}

fn parse_hits(body: &Value) -> (Vec<Hit>, String) {
    let mode = body["mode"].as_str().unwrap_or("?").to_string();
    let hits = body["results"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    Some(Hit {
                        title: r["title"].as_str().unwrap_or("(untitled)").to_string(),
                        module: r["module"].as_str().unwrap_or_default().to_string(),
                        entity_type: r["type"].as_str().unwrap_or_default().to_string(),
                        id: r["id"].as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    (hits, mode)
}

/// Run one hybrid search against the in-process API.
pub async fn run_query(api: &InProcessApi, token: Option<&str>, query: &str) -> (Vec<Hit>, String) {
    let uri = format!("/api/search?q={}&limit=20", urlencode(query));
    let response = api.get(&uri, token).await;
    if !response.is_success() {
        return (Vec::new(), format!("error {}", response.status));
    }
    parse_hits(&response.body)
}

pub fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_string()
            } else {
                c.to_string().bytes().map(|b| format!("%{b:02X}")).collect()
            }
        })
        .collect()
}

impl SearchPane {
    pub fn new(api: InProcessApi, token: Option<String>) -> Self {
        Self {
            api,
            token,
            query: String::new(),
            selected: 0,
            results: Arc::default(),
        }
    }

    /// Feed one key; Enter fires the query on a tokio task (the event loop
    /// runs inside the runtime), results land in the shared state.
    pub fn on_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Enter => {
                let query = self.query.trim().to_string();
                if query.is_empty() {
                    return true;
                }
                if let Ok(mut r) = self.results.lock() {
                    r.busy = true;
                }
                let (api, token, results) =
                    (self.api.clone(), self.token.clone(), self.results.clone());
                self.selected = 0;
                tokio::spawn(async move {
                    let (hits, mode) = run_query(&api, token.as_deref(), &query).await;
                    if let Ok(mut r) = results.lock() {
                        r.hits = hits;
                        r.mode = mode;
                        r.busy = false;
                    }
                });
            }
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => {
                let n = self.results.lock().map(|r| r.hits.len()).unwrap_or(0);
                if n > 0 {
                    self.selected = (self.selected + 1).min(n - 1);
                }
            }
            KeyCode::Char(c) => self.query.push(c),
            _ => return false,
        }
        true
    }

    pub fn render_lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from(vec![
            Span::styled("recall ▸ ".to_string(), theme.title()),
            Span::styled(self.query.clone(), theme.text()),
            Span::styled(
                " ".to_string(),
                Style::default().add_modifier(Modifier::REVERSED),
            ),
        ])];
        let Ok(r) = self.results.lock() else {
            return lines;
        };
        if r.busy {
            lines.push(Line::styled("searching…".to_string(), theme.muted()));
            return lines;
        }
        if !r.mode.is_empty() {
            lines.push(Line::styled(
                format!("{} · {} hits", r.mode, r.hits.len()),
                theme.muted(),
            ));
        }
        for (i, hit) in r.hits.iter().enumerate() {
            let style = if i == self.selected {
                theme.active_item()
            } else {
                theme.text()
            };
            lines.push(Line::from(vec![
                Span::styled(hit.title.clone(), style),
                Span::styled(
                    format!("  {} · {}", hit.module, hit.entity_type),
                    theme.muted(),
                ),
            ]));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hits_and_mode_from_the_api_shape() {
        let body = serde_json::json!({
            "query": "q", "mode": "lexical",
            "results": [{"id": "e1", "title": "Ship it", "module": "tasks", "type": "task"}]
        });
        let (hits, mode) = parse_hits(&body);
        assert_eq!(mode, "lexical");
        assert_eq!(hits[0].title, "Ship it");
        assert_eq!(hits[0].module, "tasks");
    }

    #[test]
    fn urlencode_escapes_non_alphanumerics() {
        assert_eq!(urlencode("a b/c"), "a%20b%2Fc");
    }
}
