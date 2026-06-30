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

export const TASKS_MANIFEST = {
  id: 'tasks',
  name: 'Tasks / Productivity',
  icon: '✅',
  entityTypes: {
    task: {
      label: 'Task',
      plural: 'Tasks',
      display: { title: 'title', subtitle: 'label', badge: 'status' },
    },
    project: {
      label: 'Project',
      plural: 'Projects',
      display: { title: 'title', subtitle: 'description' },
    },
    schedule_block: {
      label: 'Schedule Block',
      plural: 'Schedule Blocks',
      display: { title: 'title', subtitle: 'start' },
    },
  },
  views: [
    { id: 'board', label: 'Board', kind: 'board', type: 'task', groupBy: 'status', columns: ['DRAFT', 'IN_PROGRESS', 'REVIEW', 'COMPLETED'] },
    { id: 'today', label: 'Today', kind: 'list', type: 'task', filter: { field: 'due', onOrBefore: 'today' } },
    { id: 'calendar', label: 'Calendar', kind: 'calendar', type: 'task', dateField: 'due' },
  ],
};

export const MODULE_MANIFESTS = {
  learning: LEARNING_MANIFEST,
  tasks: TASKS_MANIFEST,
};

export function getManifest(id) {
  return MODULE_MANIFESTS[id];
}
