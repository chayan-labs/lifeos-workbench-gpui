// Generic metric aggregation over `events`, driven by a manifest `metrics`
// entry (docs/MODULES.md §1: `{ id, source:'events', where, agg, bucket?, viz }`)
// - no new storage, no bespoke per-module query code. Computed client-side
// over an already-fetched event window so any module can declare a metric
// with zero backend work.
import { resolveField } from '../renderers/displayHelpers';

function matchesWhere(event, where) {
  if (!where) return true;
  return Object.entries(where).every(([field, expected]) => resolveField(event, field) === expected);
}

function bucketKey(event, bucket) {
  if (!bucket) return null;
  const ts = event.ts ? event.ts * 1000 : Date.parse(event.created_at || '');
  if (!Number.isFinite(ts)) return 'unknown';
  const date = new Date(ts);
  if (bucket === 'hour') return date.toISOString().slice(0, 13);
  if (bucket === 'day') return date.toISOString().slice(0, 10);
  if (bucket === 'month') return date.toISOString().slice(0, 7);
  return date.toISOString().slice(0, 10);
}

function aggregate(group, agg) {
  if (agg === 'count') return group.length;
  const [kind, field] = agg.split(':');
  const values = group.map((e) => Number(resolveField(e, field))).filter(Number.isFinite);
  if (kind === 'sum') return values.reduce((a, b) => a + b, 0);
  if (kind === 'avg') return values.length ? values.reduce((a, b) => a + b, 0) / values.length : 0;
  return group.length;
}

// Funnel: `metric.stages = [{ label, where }]`, ordered. Each stage's value
// is the count of events matching that stage's filter (independently, not
// narrowed by prior stages - a simple "how many reached this event type").
function computeFunnel(events, metric) {
  return (metric.stages || []).map((stage) => ({
    label: stage.label,
    value: events.filter((e) => matchesWhere(e, stage.where)).length,
  }));
}

// Categorical breakdown: `metric.groupBy` buckets by a field's distinct
// values instead of time (e.g. events_by_type for a bar chart).
function computeGroupBy(events, metric) {
  const filtered = events.filter((e) => matchesWhere(e, metric.where));
  const groups = new Map();
  for (const e of filtered) {
    const key = resolveField(e, metric.groupBy) ?? 'unknown';
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(e);
  }
  return [...groups.entries()]
    .map(([label, group]) => ({ label, value: aggregate(group, metric.agg || 'count') }))
    .sort((a, b) => b.value - a.value);
}

function computeBucketed(events, metric) {
  const filtered = events.filter((e) => matchesWhere(e, metric.where));
  const groups = new Map();
  for (const e of filtered) {
    const key = bucketKey(e, metric.bucket) ?? 'total';
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(e);
  }
  return [...groups.entries()]
    .map(([label, group]) => ({ label, value: aggregate(group, metric.agg || 'count') }))
    .sort((a, b) => a.label.localeCompare(b.label));
}

export function computeMetric(events, metric) {
  if (metric.viz === 'funnel' || metric.stages) return computeFunnel(events, metric);
  if (metric.groupBy) return computeGroupBy(events, metric);
  return computeBucketed(events, metric);
}
