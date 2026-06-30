// Static registry of day-1 module manifests (docs/MODULES.md §1). These are
// shipped with the app (not hot-installed via self-extension - that's
// moduleRegistry.js's job for AI-built modules), but follow exactly the same
// manifest shape so they render through the same generic renderers with zero
// bespoke view code.
//
// Each manifest: { id, name, icon, entityTypes: { <type>: {label, plural,
// display} }, views: [{id, label, kind, type, dateField?, groupBy?}] }.
// `kind` selects which core/renderers/Generic* component renders the view.

export const LEARNING_MANIFEST = {
  id: 'learning',
  name: 'Learning / Study',
  icon: '🧠',
  entityTypes: {
    domain: {
      label: 'Domain',
      plural: 'Domains',
      display: { title: 'title', subtitle: 'tagline', badge: 'icon' },
    },
    topic: {
      label: 'Topic',
      plural: 'Topics',
      display: { title: 'title', subtitle: 'level', badge: 'status' },
    },
    subtopic: {
      label: 'Subtopic',
      plural: 'Subtopics',
      display: { title: 'title' },
    },
    resource: {
      label: 'Resource',
      plural: 'Resources',
      display: { title: 'title', subtitle: 'url', badge: 'kind' },
    },
    gap: {
      label: 'Gap',
      plural: 'Gaps',
      display: { title: 'title', subtitle: 'description', badge: 'severity' },
    },
    question: {
      label: 'Question',
      plural: 'Questions',
      display: { title: 'title' },
    },
  },
  views: [
    { id: 'atlas', label: 'Atlas (domains)', kind: 'list', type: 'domain' },
    { id: 'topics', label: 'Topics', kind: 'table', type: 'topic', columns: [
      { key: 'title', label: 'Topic', truncate: true },
      { key: 'level', label: 'Level' },
      { key: 'status', label: 'Status', editable: true },
    ] },
    { id: 'gaps', label: 'Gaps inbox', kind: 'list', type: 'gap' },
    { id: 'due', label: "What's due", kind: 'calendar', type: 'topic', dateField: 'next_due' },
  ],
};

export const MODULE_MANIFESTS = {
  learning: LEARNING_MANIFEST,
};

export function getManifest(id) {
  return MODULE_MANIFESTS[id];
}
