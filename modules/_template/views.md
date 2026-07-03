# Views and entity types - the rendering contract

Everything a module renders comes from two manifest fields: `entityTypes` (what the data looks like) and `views` (how it's arranged on screen).
The frontend's generic renderer system (`frontend/src/core/`) consumes exactly this shape - there is never a per-module React component.

## `entityTypes`

```js
entityTypes: {
  <typeId>: {
    label,            // singular display name, e.g. "Task"
    plural,           // e.g. "Tasks"
    icon,             // optional Lucide icon name
    attrs: {
      <field>: { type: 'text'|'number'|'date'|'enum'|'ref'|'bool'|'secret'|'blob', enum?, ref?, required? },
    },
    display: { title, subtitle?, badge? },  // which attrs/entity fields drive the UI
    lifecycle: [/* status strings, e.g. ['active', 'archived'] */],
  },
},
```

`display.title`/`subtitle`/`badge` are each either a plain field name (checked on the entity first, then `entity.attrs`) or a resolver function `(entity) => value` for a derived value.
This is the only place a module's data shape is connected to what shows up on screen - the renderer never hardcodes a field name.

## `views`

```js
views: [
  {
    id, label,
    kind: 'list' | 'board' | 'table' | 'calendar' | 'detail' | 'graph' | 'gallery' | 'timeline' | 'map' | 'metric',
    type,        // which entityTypes key this view lists
    groupBy?,    // 'board': the status/lifecycle field to group columns by
    sortBy?,
    filter?,     // e.g. { field: 'due', onOrBefore: 'today' } - narrows entities client-side
    columns?,    // 'table': [{ key, label, editable? }]; 'board': the column values to render
    dateField?,  // 'calendar': which attrs field holds the date
    mediaField?, // 'gallery': which attrs field holds the image/blob URL
    metric?,     // 'metric': id of an entry in the manifest's `metrics` array
  },
],
```

### View kind → renderer

| `kind` | Renderer | Used for |
| --- | --- | --- |
| `list` | `GenericList` | A simple feed - inbox items, gaps, setups. |
| `board` | `GenericBoard` | Kanban, grouped by a lifecycle/status field. Moves PATCH the entity optimistically. |
| `table` | `GenericTable` | Spreadsheet-like rows with inline-editable cells. |
| `calendar` | `GenericCalendar` | Agenda grouped by the ISO-date prefix of `dateField`. |
| `gallery` | `GenericGallery` | Thumbnail grid; only renders direct `http(s)://` media URLs, an honest placeholder otherwise. |
| `metric` | `GenericMetricChart` | Line/bar/funnel charts computed client-side over the `events` log per a `metrics` entry - no new storage. |
| `detail` / `graph` / `timeline` / `map` | `GenericDetail` / Cytoscape graph view / `GenericTimeline` / `GenericMap` | Single-entity inspector, cross-entity graph, chronological feed, geo pins. |

A module needs zero of these to be valid (see the `_template`'s own single `list` view for the minimum), and can declare as many as its `entityTypes` warrant - every day-1 module (`frontend/src/lib/moduleManifests.js`) follows exactly this contract.

## Example: this template's view

```js
views: [
  { id: 'all', label: 'All Items', kind: 'list', type: 'item', sortBy: 'created_at' },
],
```

One `entityTypes.item` (a `name` + `notes` text pair) rendered as a single list, sorted newest-first.
That is a complete, working module.
