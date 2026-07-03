//! Renderer-agnostic Life OS view-models.
//!
//! This is the pure half of the origin `views.rs` - the manifest `view.kind`
//! dispatch and the `List`/`Board`/`Detail` builders - lifted out of its ratatui
//! `lines()` methods so the gpui panes can consume the same view-models the
//! ratatui frontend does. The interpretation is identical: manifests are never
//! forked or specialised, and an unknown `view.kind` degrades to `list` (the
//! honest fallback per DESIGN.md), never to a manifest fork.

use crate::manifest::{EntityType, View};
use serde_json::Value;

/// A view.kind dispatched against its renderer.
#[derive(Debug, Clone)]
pub enum Rendered {
    List(ListView),
    Board(BoardView),
    Detail(DetailView),
    Table(TableView),
    Calendar(CalendarView),
    Gallery(GalleryView),
    Timeline(TimelineView),
    Map(MapView),
}

/// Entry point: pick the view-model for a manifest view. Unknown kinds degrade
/// to `list` - `graph` (the learning module's "Knowledge Tree") included,
/// matching the upstream React app's own `KIND_RENDERERS[kind] || GenericList`
/// fallback (it has no graph renderer either), so this is parity, not a gap.
pub fn dispatch(view: &View, entity_type: &EntityType, entities: &[Value]) -> Rendered {
    let filtered = apply_filter(view.filter.as_deref(), entities);
    match view.kind.as_str() {
        "board" => Rendered::Board(BoardView::build(view, entity_type, &filtered)),
        "detail" => Rendered::Detail(DetailView::build(entity_type, filtered.first())),
        "table" => Rendered::Table(TableView::build(entity_type, &filtered)),
        "calendar" => Rendered::Calendar(CalendarView::build(entity_type, &filtered)),
        "gallery" => Rendered::Gallery(GalleryView::build(entity_type, &filtered)),
        "timeline" => Rendered::Timeline(TimelineView::build(entity_type, &filtered)),
        "map" => Rendered::Map(MapView::build(entity_type, &filtered)),
        // "graph" and anything else: honest list fallback.
        _ => Rendered::List(ListView::build(entity_type, &filtered)),
    }
}

/// Find the first attr whose declared `type` matches one of `types`, honouring
/// `entity_type.attrs`' declaration order isn't guaranteed (it's a HashMap), so
/// this sorts by name for determinism. Manifests don't declare a per-view date/
/// image/location field, so kinds that need one (calendar/gallery/timeline/map)
/// infer it from the entity type's own attr types - the same kind of heuristic
/// `ListView` already applies via `display`.
fn find_attr_by_type(entity_type: &EntityType, types: &[&str]) -> Option<String> {
    let mut names: Vec<&String> = entity_type.attrs.keys().collect();
    names.sort();
    names
        .into_iter()
        .find(|name| {
            entity_type
                .attrs
                .get(*name)
                .is_some_and(|a| types.contains(&a.attr_type.as_str()))
        })
        .cloned()
}

/// Find the first attr whose name matches one of `candidates` (case-insensitive
/// exact match), used when type-based detection isn't enough (e.g. an image
/// attr is often typed as a plain "string" holding a URL/path).
fn find_attr_by_name(entity_type: &EntityType, candidates: &[&str]) -> Option<String> {
    let mut names: Vec<&String> = entity_type.attrs.keys().collect();
    names.sort();
    names
        .into_iter()
        .find(|name| candidates.iter().any(|c| c.eq_ignore_ascii_case(name)))
        .cloned()
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
pub fn field_of(entity: &Value, field: &str) -> Option<String> {
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

#[derive(Debug, Clone)]
pub struct ListRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub badge: String,
}

/// Dense one-line rows keyed off the entity type's `display` fields. Also the
/// degradation target for unknown kinds.
#[derive(Debug, Clone)]
pub struct ListView {
    pub rows: Vec<ListRow>,
    /// The `display` field names, surfaced so the gpui table can label columns.
    pub title_label: String,
    pub subtitle_label: String,
    pub badge_label: String,
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
        ListView {
            rows,
            title_label: d.title.clone().unwrap_or_else(|| "title".into()),
            subtitle_label: d.subtitle.clone().unwrap_or_default(),
            badge_label: d.badge.clone().unwrap_or_default(),
        }
    }
}

// --------------------------------------------------------------------- board

#[derive(Debug, Clone)]
pub struct BoardColumn {
    pub status: String,
    pub cards: Vec<ListRow>,
}

/// Kanban as columns keyed by the lifecycle field. A card move is a status
/// change the caller persists via the in-process API (`PATCH /api/entity/:id`).
#[derive(Debug, Clone)]
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

    /// The status a card lands in when moved one column right/left. Returns
    /// `(entity_id, new_status)` for the caller to persist.
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

/// Stacked `label: value` fields + a markdown body region.
#[derive(Debug, Clone)]
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
}

// ---------------------------------------------------------------------- table

/// A row of arbitrary label/value pairs, in a fixed column order shared by
/// every row (unlike `DetailView`, which is one entity's whole field stack).
#[derive(Debug, Clone)]
pub struct TableRow {
    pub id: String,
    pub cells: Vec<String>,
}

/// A denser, multi-column sibling of `ListView`: every declared attr becomes a
/// column (manifests don't declare an explicit per-view column set, so this
/// mirrors `DetailView`'s "every attr, sorted" heuristic rather than just the
/// 3 `display` fields `ListView` shows).
#[derive(Debug, Clone)]
pub struct TableView {
    pub columns: Vec<String>,
    pub rows: Vec<TableRow>,
}

impl TableView {
    fn build(entity_type: &EntityType, entities: &[Value]) -> TableView {
        let mut columns = vec!["title".to_string()];
        let mut attr_names: Vec<&String> = entity_type.attrs.keys().collect();
        attr_names.sort();
        columns.extend(attr_names.iter().map(|s| s.to_string()));

        let rows = entities
            .iter()
            .map(|e| TableRow {
                id: field_of(e, "id").unwrap_or_default(),
                cells: columns
                    .iter()
                    .map(|c| field_of(e, c).unwrap_or_default())
                    .collect(),
            })
            .collect();
        TableView { columns, rows }
    }
}

// ------------------------------------------------------------------- calendar

#[derive(Debug, Clone)]
pub struct CalendarEntry {
    pub id: String,
    pub title: String,
    /// `YYYY-MM-DD` if the date attr parsed cleanly, else the raw value.
    pub date: String,
}

/// Entities bucketed by their inferred date attr, day-ascending. The gpui
/// pane lays these out as a month grid; this view-model just groups and sorts.
#[derive(Debug, Clone)]
pub struct CalendarView {
    pub date_field: Option<String>,
    pub entries: Vec<CalendarEntry>,
}

impl CalendarView {
    fn build(entity_type: &EntityType, entities: &[Value]) -> CalendarView {
        let date_field = find_attr_by_type(entity_type, &["date", "datetime"])
            .or_else(|| find_attr_by_name(entity_type, &["due", "date", "start", "when"]));
        let mut entries: Vec<CalendarEntry> = entities
            .iter()
            .filter_map(|e| {
                let date = date_field.as_deref().and_then(|f| field_of(e, f))?;
                Some(CalendarEntry {
                    id: field_of(e, "id").unwrap_or_default(),
                    title: field_of(e, "title").unwrap_or_default(),
                    date,
                })
            })
            .collect();
        entries.sort_by(|a, b| a.date.cmp(&b.date));
        CalendarView {
            date_field,
            entries,
        }
    }
}

// -------------------------------------------------------------------- gallery

#[derive(Debug, Clone)]
pub struct GalleryCard {
    pub id: String,
    pub title: String,
    /// A local path or URL if an image-like attr was found; `None` degrades to
    /// a text-only card rather than a broken image icon.
    pub image: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GalleryView {
    pub cards: Vec<GalleryCard>,
}

impl GalleryView {
    fn build(entity_type: &EntityType, entities: &[Value]) -> GalleryView {
        let image_field = find_attr_by_name(
            entity_type,
            &["image", "thumbnail", "photo", "cover", "asset", "preview"],
        );
        let cards = entities
            .iter()
            .map(|e| GalleryCard {
                id: field_of(e, "id").unwrap_or_default(),
                title: field_of(e, "title").unwrap_or_default(),
                image: image_field
                    .as_deref()
                    .and_then(|f| field_of(e, f))
                    .filter(|s| !s.is_empty()),
            })
            .collect();
        GalleryView { cards }
    }
}

// ------------------------------------------------------------------- timeline

#[derive(Debug, Clone)]
pub struct TimelineEntry {
    pub id: String,
    pub title: String,
    pub date: String,
    pub subtitle: String,
}

/// Date-sorted entries for a vertical timeline layout.
#[derive(Debug, Clone)]
pub struct TimelineView {
    pub entries: Vec<TimelineEntry>,
}

impl TimelineView {
    fn build(entity_type: &EntityType, entities: &[Value]) -> TimelineView {
        let date_field = find_attr_by_type(entity_type, &["date", "datetime"])
            .or_else(|| find_attr_by_name(entity_type, &["due", "date", "start", "when"]));
        let mut entries: Vec<TimelineEntry> = entities
            .iter()
            .map(|e| TimelineEntry {
                id: field_of(e, "id").unwrap_or_default(),
                title: field_of(e, "title").unwrap_or_default(),
                date: date_field
                    .as_deref()
                    .and_then(|f| field_of(e, f))
                    .unwrap_or_default(),
                subtitle: entity_type
                    .display
                    .subtitle
                    .as_deref()
                    .and_then(|f| field_of(e, f))
                    .unwrap_or_default(),
            })
            .collect();
        entries.sort_by(|a, b| a.date.cmp(&b.date));
        TimelineView { entries }
    }
}

// ------------------------------------------------------------------------ map

#[derive(Debug, Clone)]
pub struct MapEntry {
    pub id: String,
    pub title: String,
    /// Coordinates or address shown as text - a real interactive map/tile
    /// provider is out of scope (an honest structured list beats a fake map).
    pub location: String,
}

#[derive(Debug, Clone)]
pub struct MapView {
    pub entries: Vec<MapEntry>,
}

impl MapView {
    fn build(entity_type: &EntityType, entities: &[Value]) -> MapView {
        let lat_field = find_attr_by_name(entity_type, &["lat", "latitude"]);
        let lng_field = find_attr_by_name(entity_type, &["lng", "lon", "longitude"]);
        let addr_field = find_attr_by_name(entity_type, &["address", "location", "place"]);

        let mut entries: Vec<MapEntry> = entities
            .iter()
            .filter_map(|e| {
                let location = match (&lat_field, &lng_field) {
                    (Some(lat_f), Some(lng_f)) => {
                        let lat = field_of(e, lat_f)?;
                        let lng = field_of(e, lng_f)?;
                        format!("{lat}, {lng}")
                    }
                    _ => addr_field.as_deref().and_then(|f| field_of(e, f))?,
                };
                Some(MapEntry {
                    id: field_of(e, "id").unwrap_or_default(),
                    title: field_of(e, "title").unwrap_or_default(),
                    location,
                })
            })
            .collect();
        entries.sort_by(|a, b| a.title.cmp(&b.title));
        MapView { entries }
    }
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
        (
            m.views[0].clone(),
            m.views[1].clone(),
            m.entity_types["task"].clone(),
        )
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
        assert_eq!(v.title_label, "title");
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
    fn graph_kind_degrades_to_list_matching_upstreams_own_fallback() {
        let (mut view, _, et) = tasks_manifest();
        view.kind = "graph".into();
        view.group_by = None;
        view.filter = None;
        assert!(matches!(
            dispatch(&view, &et, &entities()),
            Rendered::List(_)
        ));
    }

    fn travel_manifest() -> (View, EntityType) {
        let m = parse_manifest(
            r#"osRegisterModule({
                id: "travel", name: "Travel",
                entityTypes: { trip: {
                    label: "Trip",
                    attrs: {
                        start: { type: "date" },
                        latitude: { type: "number" },
                        longitude: { type: "number" },
                        cover: { type: "string" }
                    },
                    display: { title: "title", subtitle: "start" }
                }},
                views: [
                    { id: "t", label: "Table", kind: "table", type: "trip" },
                    { id: "c", label: "Calendar", kind: "calendar", type: "trip" },
                    { id: "g", label: "Gallery", kind: "gallery", type: "trip" },
                    { id: "tl", label: "Timeline", kind: "timeline", type: "trip" },
                    { id: "m", label: "Map", kind: "map", type: "trip" }
                ]
            });"#,
        )
        .unwrap();
        (m.views[0].clone(), m.entity_types["trip"].clone())
    }

    fn trips() -> Vec<Value> {
        vec![
            json!({"id": "t1", "title": "Kyoto", "attrs": {
                "start": "2026-03-02", "latitude": "35.0", "longitude": "135.7", "cover": "kyoto.jpg"
            }}),
            json!({"id": "t2", "title": "Reykjavik", "attrs": {
                "start": "2026-01-10", "latitude": "64.1", "longitude": "-21.9"
            }}),
        ]
    }

    #[test]
    fn table_view_has_a_column_per_declared_attr() {
        let (_, et) = travel_manifest();
        let mut view = travel_manifest().0;
        view.kind = "table".into();
        let Rendered::Table(v) = dispatch(&view, &et, &trips()) else {
            panic!("expected table");
        };
        assert!(v.columns.contains(&"latitude".to_string()));
        assert_eq!(v.rows.len(), 2);
        assert_eq!(v.rows[0].id, "t1");
    }

    #[test]
    fn calendar_view_infers_the_date_attr_and_sorts_ascending() {
        let (_, et) = travel_manifest();
        let mut view = travel_manifest().0;
        view.kind = "calendar".into();
        let Rendered::Calendar(v) = dispatch(&view, &et, &trips()) else {
            panic!("expected calendar");
        };
        assert_eq!(v.date_field.as_deref(), Some("start"));
        assert_eq!(v.entries[0].id, "t2", "earlier date sorts first");
    }

    #[test]
    fn gallery_view_finds_an_image_attr_and_honestly_degrades_without_one() {
        let (_, et) = travel_manifest();
        let mut view = travel_manifest().0;
        view.kind = "gallery".into();
        let Rendered::Gallery(v) = dispatch(&view, &et, &trips()) else {
            panic!("expected gallery");
        };
        assert_eq!(v.cards[0].image.as_deref(), Some("kyoto.jpg"));
        assert_eq!(v.cards[1].image, None, "no cover attr on this entity");
    }

    #[test]
    fn timeline_view_sorts_by_inferred_date() {
        let (_, et) = travel_manifest();
        let mut view = travel_manifest().0;
        view.kind = "timeline".into();
        let Rendered::Timeline(v) = dispatch(&view, &et, &trips()) else {
            panic!("expected timeline");
        };
        assert_eq!(v.entries[0].id, "t2");
    }

    #[test]
    fn map_view_renders_coordinates_as_text_not_a_fake_map() {
        let (_, et) = travel_manifest();
        let mut view = travel_manifest().0;
        view.kind = "map".into();
        let Rendered::Map(v) = dispatch(&view, &et, &trips()) else {
            panic!("expected map");
        };
        assert_eq!(
            v.entries.iter().find(|e| e.id == "t1").unwrap().location,
            "35.0, 135.7"
        );
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
    }
}
