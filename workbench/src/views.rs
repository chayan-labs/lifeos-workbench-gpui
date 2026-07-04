//! The renderer backend (the core fork move): interprets a manifest `view`
//! by `kind` over entities fetched from `lifeos-api` in-process, producing
//! pure view-models the shell draws with ratatui. Manifests are never forked
//! or TUI-specialized; a `view.kind` this renderer set doesn't know yet
//! degrades to the list renderer per DESIGN.md, it never mutates the module.

use crate::manifest::{EntityType, View};
use crate::markdown;
use crate::theme::{Theme, ACCENT, FG_DIM, PRIMARY};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

/// A view.kind dispatched against its renderer.
#[derive(Debug)]
pub enum Rendered {
    List(ListView),
    Board(BoardView),
    Detail(DetailView),
}

/// Entry point: pick the renderer for a manifest view. Unknown kinds
/// degrade to `list` (the honest fallback), never to a manifest fork.
pub fn dispatch(view: &View, entity_type: &EntityType, entities: &[Value]) -> Rendered {
    let filtered = apply_filter(view.filter.as_deref(), entities);
    match view.kind.as_str() {
        "board" => Rendered::Board(BoardView::build(view, entity_type, &filtered)),
        "detail" => Rendered::Detail(DetailView::build(entity_type, filtered.first())),
        _ => Rendered::List(ListView::build(entity_type, &filtered)),
    }
}

/// Minimal manifest filter support: `field = 'value'` (the shape upstream
/// manifests actually use). Anything unparsable filters nothing.
fn apply_filter(filter: Option<&str>, entities: &[Value]) -> Vec<Value> {
    let Some((field, value)) = filter.and_then(parse_eq_filter) else {
        return entities.to_vec();
    };
    entities
        .iter()
        .filter(|e| field_of(e, &field).as_deref() == Some(value.as_str()))
        .cloned()
        .collect()
}

fn parse_eq_filter(filter: &str) -> Option<(String, String)> {
    let (field, value) = filter.split_once('=')?;
    Some((
        field.trim().to_string(),
        value
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_string(),
    ))
}

/// Look up a display field: top-level entity column first, then attrs.
fn field_of(entity: &Value, field: &str) -> Option<String> {
    let v = match &entity[field] {
        Value::Null => &entity["attrs"][field],
        v => v,
    };
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

// ---------------------------------------------------------------- list/table

#[derive(Debug)]
pub struct ListRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub badge: String,
}

/// DESIGN.md component 2: dense one-line rows, reverse-video cursor,
/// bold+dim headers. Also the degradation target for unknown kinds.
#[derive(Debug)]
pub struct ListView {
    pub rows: Vec<ListRow>,
    pub cursor: usize,
}

impl ListView {
    fn build(entity_type: &EntityType, entities: &[Value]) -> ListView {
        let d = &entity_type.display;
        let rows = entities
            .iter()
            .map(|e| ListRow {
                id: field_of(e, "id").unwrap_or_default(),
                title: d
                    .title
                    .as_deref()
                    .and_then(|f| field_of(e, f))
                    .unwrap_or_default(),
                subtitle: d
                    .subtitle
                    .as_deref()
                    .and_then(|f| field_of(e, f))
                    .unwrap_or_default(),
                badge: d
                    .badge
                    .as_deref()
                    .and_then(|f| field_of(e, f))
                    .unwrap_or_default(),
            })
            .collect();
        ListView { rows, cursor: 0 }
    }

    pub fn lines(&self, entity_label: &str, theme: &Theme) -> Vec<Line<'static>> {
        let header = Style::default()
            .fg(FG_DIM.resolve(theme.support))
            .add_modifier(Modifier::BOLD);
        let mut lines = vec![Line::styled(
            format!("{entity_label:<40} {:<16} badge", "sub"),
            header,
        )];
        for (i, row) in self.rows.iter().enumerate() {
            let style = if i == self.cursor {
                theme.active_item()
            } else {
                theme.text()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{:<40} ", truncate(&row.title, 39)), style),
                Span::styled(format!("{:<16} ", truncate(&row.subtitle, 15)), style),
                Span::styled(row.badge.clone(), style.add_modifier(Modifier::BOLD)),
            ]));
        }
        lines
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

// --------------------------------------------------------------------- board

#[derive(Debug)]
pub struct BoardColumn {
    pub status: String,
    pub cards: Vec<ListRow>,
}

/// DESIGN.md component 3: Kanban as side-by-side columns keyed by the
/// lifecycle field; a card move is a status change the caller persists via
/// the in-process API (`PATCH /api/entity/:id`).
#[derive(Debug)]
pub struct BoardView {
    pub columns: Vec<BoardColumn>,
}

impl BoardView {
    fn build(view: &View, entity_type: &EntityType, entities: &[Value]) -> BoardView {
        let group_by = view.group_by.as_deref().unwrap_or("status");
        let columns = entity_type
            .lifecycle
            .iter()
            .map(|status| BoardColumn {
                status: status.clone(),
                cards: entities
                    .iter()
                    .filter(|e| field_of(e, group_by).as_deref() == Some(status.as_str()))
                    .map(|e| ListRow {
                        id: field_of(e, "id").unwrap_or_default(),
                        title: entity_type
                            .display
                            .title
                            .as_deref()
                            .and_then(|f| field_of(e, f))
                            .unwrap_or_default(),
                        subtitle: String::new(),
                        badge: entity_type
                            .display
                            .badge
                            .as_deref()
                            .and_then(|f| field_of(e, f))
                            .unwrap_or_default(),
                    })
                    .collect(),
            })
            .collect();
        BoardView { columns }
    }

    /// The status a card lands in when moved one column right/left.
    /// Returns `(entity_id, new_status)` for the caller to persist.
    pub fn move_card(&self, id: &str, delta: isize) -> Option<(String, String)> {
        let col = self
            .columns
            .iter()
            .position(|c| c.cards.iter().any(|card| card.id == id))?;
        let target = col as isize + delta;
        if target < 0 || target as usize >= self.columns.len() {
            return None;
        }
        Some((id.to_string(), self.columns[target as usize].status.clone()))
    }
}

// -------------------------------------------------------------------- detail

/// DESIGN.md component 5: stacked `label: value` fields + a styled-markdown
/// body region.
#[derive(Debug)]
pub struct DetailView {
    pub fields: Vec<(String, String)>,
    pub body: String,
}

impl DetailView {
    fn build(entity_type: &EntityType, entity: Option<&Value>) -> DetailView {
        let Some(entity) = entity else {
            return DetailView {
                fields: Vec::new(),
                body: String::new(),
            };
        };
        let mut fields = vec![
            (
                "title".to_string(),
                field_of(entity, "title").unwrap_or_default(),
            ),
            (
                "status".to_string(),
                field_of(entity, "status").unwrap_or_default(),
            ),
        ];
        let mut attr_names: Vec<&String> = entity_type.attrs.keys().collect();
        attr_names.sort();
        for name in attr_names {
            if let Some(v) = field_of(entity, name) {
                fields.push((name.clone(), v));
            }
        }
        let body = field_of(entity, "body")
            .or_else(|| field_of(entity, "content"))
            .unwrap_or_default();
        DetailView { fields, body }
    }

    pub fn lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        let label = Style::default().fg(FG_DIM.resolve(theme.support));
        let mut lines: Vec<Line> = self
            .fields
            .iter()
            .map(|(k, v)| {
                Line::from(vec![
                    Span::styled(format!("{k:>12}: "), label),
                    Span::styled(v.clone(), theme.text()),
                ])
            })
            .collect();
        if !self.body.is_empty() {
            lines.push(Line::styled(
                "─".repeat(40),
                Style::default().fg(PRIMARY.resolve(theme.support)),
            ));
            lines.extend(markdown::render(&self.body, theme));
        }
        lines
    }
}

// keep ACCENT referenced for badge styling growth without a warning
#[allow(dead_code)]
fn _accent_anchor(theme: &Theme) -> Style {
    Style::default().fg(ACCENT.resolve(theme.support))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::parse_manifest;
    use serde_json::json;

    fn tasks_manifest() -> (View, View, EntityType) {
        let m = parse_manifest(
            r#"osRegisterModule({
                id: "tasks", name: "Tasks",
                entityTypes: { task: {
                    label: "Task",
                    attrs: { priority: { type: "enum", enum: ["high","low"] }, due: { type: "date" } },
                    display: { title: "title", subtitle: "due", badge: "priority" },
                    lifecycle: ["todo", "in_progress", "completed"]
                }},
                views: [
                    { id: "kanban", label: "Board", kind: "board", type: "task", groupBy: "status" },
                    { id: "today", label: "Today", kind: "list", type: "task", filter: "status = 'in_progress'" }
                ]
            });"#,
        )
        .unwrap();
        let board = m.views[0].clone();
        let list = m.views[1].clone();
        let et = m.entity_types["task"].clone();
        (board, list, et)
    }

    fn entities() -> Vec<Value> {
        vec![
            json!({"id": "e1", "title": "Ship fork", "status": "in_progress",
                   "attrs": {"priority": "high", "due": "2026-07-10"}}),
            json!({"id": "e2", "title": "Write docs", "status": "todo",
                   "attrs": {"priority": "low"}}),
        ]
    }

    #[test]
    fn list_view_filters_by_manifest_filter_and_maps_display_fields() {
        let (_, list, et) = tasks_manifest();
        let Rendered::List(v) = dispatch(&list, &et, &entities()) else {
            panic!("expected list");
        };
        assert_eq!(v.rows.len(), 1);
        assert_eq!(v.rows[0].title, "Ship fork");
        assert_eq!(v.rows[0].subtitle, "2026-07-10");
        assert_eq!(v.rows[0].badge, "high");
    }

    #[test]
    fn board_groups_by_lifecycle_and_move_card_yields_persistable_change() {
        let (board, _, et) = tasks_manifest();
        let Rendered::Board(v) = dispatch(&board, &et, &entities()) else {
            panic!("expected board");
        };
        assert_eq!(v.columns.len(), 3);
        assert_eq!(v.columns[0].cards[0].id, "e2");
        assert_eq!(v.columns[1].cards[0].id, "e1");
        assert_eq!(
            v.move_card("e1", 1),
            Some(("e1".into(), "completed".into()))
        );
        assert_eq!(
            v.move_card("e2", -1),
            None,
            "cannot move left of first column"
        );
    }

    #[test]
    fn unknown_view_kind_degrades_to_list_never_forks_the_manifest() {
        let (mut view, _, et) = tasks_manifest();
        view.kind = "gallery".into();
        view.group_by = None;
        view.filter = None;
        assert!(matches!(
            dispatch(&view, &et, &entities()),
            Rendered::List(_)
        ));
    }

    #[test]
    fn detail_renders_fields_and_markdown_body() {
        let (_, _, et) = tasks_manifest();
        let entity = json!({"id": "e3", "title": "Note", "status": "todo",
                            "attrs": {"body": "# Heading\n- point"}});
        let view = View {
            id: "d".into(),
            label: "Detail".into(),
            kind: "detail".into(),
            entity_type: "task".into(),
            group_by: None,
            filter: None,
        };
        let Rendered::Detail(v) = dispatch(&view, &et, &[entity]) else {
            panic!("expected detail");
        };
        assert_eq!(v.fields[0], ("title".to_string(), "Note".to_string()));
        assert_eq!(v.body, "# Heading\n- point");
        let theme = Theme::new(crate::theme::ColorSupport::TrueColor);
        let text: Vec<String> = v
            .lines(&theme)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert!(text.iter().any(|l| l.contains("Heading")));
    }
}
