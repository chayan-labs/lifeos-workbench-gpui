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

export const CODING_MANIFEST = {
  id: 'coding',
  name: 'Coding / Projects',
  icon: '🛠️',
  entityTypes: {
    project: {
      label: 'Project',
      plural: 'Projects',
      display: { title: 'title' },
    },
    repo: {
      label: 'Repo',
      plural: 'Repos',
      // ci_state is local-only until GitHub data flows in via Nango (Phase 3)
      display: { title: 'title', subtitle: (e) => e.attrs?.remote, badge: 'status' },
    },
    gap: {
      label: 'Gap',
      plural: 'Gaps',
      display: { title: 'title', subtitle: 'description' },
    },
    ci_run: {
      label: 'CI Run',
      plural: 'CI Runs',
      display: { title: 'title', badge: 'status' },
    },
    review: {
      label: 'Review',
      plural: 'Reviews',
      display: { title: 'title' },
    },
  },
  views: [
    { id: 'status', label: 'Status', kind: 'table', type: 'repo', columns: [
      { key: 'title', label: 'Repo' },
      { key: 'status', label: 'Status', editable: true },
      { key: 'default_branch', label: 'Branch' },
      { key: 'ci_state', label: 'CI' },
    ] },
    { id: 'board', label: 'Blocked / Active / Done', kind: 'board', type: 'repo', groupBy: 'status', columns: ['dirty', 'clean', 'blocked'] },
  ],
};

// Trading is read-only for any agent/bot by hard architectural rule
// (docs/SECURITY.md §1): there is no place/modify/cancel/GTT tool anywhere
// in the closed ACTION_TOOLS registry (lib/agentActions.js), and
// broker-guard fails closed on any such attempt at the hook layer. A
// `proposed_order` is a draft-only entity; turning it into a real order
// requires a separate, human-typed-confirmation executor that does not
// exist in this app at all (Phase 6) - the manifest below has no tool, no
// agentTool, and no button anywhere that could place one.
export const TRADING_MANIFEST = {
  id: 'trading',
  name: 'Trading',
  icon: '📈',
  entityTypes: {
    trade: {
      label: 'Trade',
      plural: 'Trades',
      display: { title: (e) => `${e.attrs?.symbol || '?'} ${e.attrs?.side || ''}`, subtitle: 'status', badge: (e) => e.attrs?.r_multiple != null ? `R ${e.attrs.r_multiple}` : null },
    },
    setup: {
      label: 'Setup / Playbook',
      plural: 'Setups',
      display: { title: 'title', subtitle: 'description' },
    },
    proposed_order: {
      label: 'Proposed Order (draft only - never auto-executes)',
      plural: 'Proposed Orders',
      display: { title: (e) => `${e.attrs?.symbol || '?'} ${e.attrs?.side || ''} x${e.attrs?.qty || '?'}`, badge: 'status' },
    },
  },
  views: [
    { id: 'journal', label: 'Journal', kind: 'table', type: 'trade', columns: [
      { key: 'symbol', label: 'Symbol' },
      { key: 'side', label: 'Side' },
      { key: 'entry', label: 'Entry' },
      { key: 'exit', label: 'Exit' },
      { key: 'stop', label: 'Stop' },
      { key: 'target', label: 'Target' },
      { key: 'r_multiple', label: 'R' },
      { key: 'pnl', label: 'PnL' },
      { key: 'emotion', label: 'Emotion', editable: true },
    ] },
    { id: 'setups', label: 'Setups', kind: 'list', type: 'setup' },
    { id: 'proposed', label: 'Proposed Orders (approve in Telegram - never auto-runs)', kind: 'list', type: 'proposed_order' },
    { id: 'equity', label: 'Equity Curve', kind: 'metric', metric: 'equity_curve' },
  ],
  metrics: [
    { id: 'equity_curve', source: 'events', where: { type: 'trade.closed' }, agg: 'sum:pnl', bucket: 'day', viz: 'line', cumulative: true },
  ],
};

// Publishing is always gated (docs/SECURITY.md §2): drafting any post/reply/
// dm goes through the single closed `draft.create` tool (lib/agentActions.js,
// #33), which is already classified `gated` regardless of which module/type
// it's drafting for - there is no separate "social.publish" tool registered
// anywhere, so a draft can only become published via a human decision (see
// Modules.jsx's decideDraft, #32). Account linking (Instagram/X/Reddit/Slack/
// WhatsApp via Nango) is explicitly deferred to Phase 3 per this issue.
export const SOCIAL_MANIFEST = {
  id: 'social',
  name: 'Social',
  icon: '💬',
  entityTypes: {
    social_account: {
      label: 'Social Account',
      plural: 'Accounts',
      display: { title: (e) => e.attrs?.handle, subtitle: 'provider' },
    },
    post: {
      label: 'Post',
      plural: 'Posts',
      display: { title: 'title', subtitle: (e) => e.attrs?.scheduled_for, badge: 'status' },
    },
    reply: {
      label: 'Reply',
      plural: 'Replies',
      display: { title: 'title', badge: 'status' },
    },
    dm: {
      label: 'DM',
      plural: 'DMs',
      display: { title: 'title', badge: 'status' },
    },
    mention: {
      label: 'Mention',
      plural: 'Mentions',
      display: { title: 'title', subtitle: (e) => e.attrs?.from },
    },
    thread: {
      label: 'Thread',
      plural: 'Threads',
      display: { title: 'title' },
    },
  },
  views: [
    { id: 'inbox', label: 'Inbox (mentions + DMs)', kind: 'list', type: 'mention' },
    { id: 'posts', label: 'Posts', kind: 'board', type: 'post', groupBy: 'status', columns: ['drafted', 'published', 'rejected'] },
    { id: 'accounts', label: 'Accounts (link via Integrations - Nango, Phase 3)', kind: 'list', type: 'social_account' },
  ],
};

// Publishing content is outward (it `publishes_to` a Social account), so it
// stays gated through the same draft.create -> human decision path Social
// uses (#43) - this manifest declares no publish tool of its own.
export const MARKETING_MANIFEST = {
  id: 'marketing',
  name: 'Marketing',
  icon: '📣',
  entityTypes: {
    campaign: {
      label: 'Campaign',
      plural: 'Campaigns',
      display: { title: 'title', subtitle: (e) => `${e.attrs?.start || '?'} → ${e.attrs?.end || '?'}`, badge: 'status' },
    },
    content: {
      label: 'Content',
      plural: 'Content',
      display: { title: 'title', subtitle: (e) => e.attrs?.channel, badge: 'status' },
    },
    audience: {
      label: 'Audience / Segment',
      plural: 'Audiences',
      display: { title: 'title' },
    },
    lead: {
      label: 'Lead',
      plural: 'Leads',
      display: { title: 'title', subtitle: (e) => e.attrs?.email, badge: 'status' },
    },
    channel: {
      label: 'Channel',
      plural: 'Channels',
      display: { title: 'title' },
    },
  },
  views: [
    { id: 'calendar', label: 'Content Calendar', kind: 'calendar', type: 'content', dateField: 'scheduled_for' },
    { id: 'campaigns', label: 'Campaigns', kind: 'list', type: 'campaign' },
    { id: 'leads', label: 'Leads', kind: 'table', type: 'lead', columns: [
      { key: 'title', label: 'Lead' },
      { key: 'email', label: 'Email' },
      { key: 'status', label: 'Status', editable: true },
    ] },
    { id: 'funnel', label: 'Funnel', kind: 'metric', metric: 'campaign_funnel' },
  ],
  metrics: [
    {
      id: 'campaign_funnel',
      source: 'events',
      viz: 'funnel',
      stages: [
        { label: 'Drafted', where: { type: 'content.drafted' } },
        { label: 'Sent', where: { type: 'content.sent' } },
        { label: 'Campaign launched', where: { type: 'campaign.launched' } },
      ],
    },
  ],
};

// mcp-figma (read+write) and mcp-higgsfield (generation) are loaded
// on-demand via mcp-multiplexer when actually invoked, never mounted
// always-on (docs/INTEGRATIONS.md token-discipline rule) - this manifest has
// no UI of its own that calls them yet (that needs an agent/console surface,
// not a view), so it stays an honest gallery + library over whatever assets
// already exist. Per-type semantic diff for design files is lifeos-vcs work,
// explicitly deferred to Phase 6.
export const DESIGN_MANIFEST = {
  id: 'design',
  name: 'Design',
  icon: '🎨',
  entityTypes: {
    design_file: {
      label: 'Design File (Figma)',
      plural: 'Design Files',
      display: { title: 'title', subtitle: (e) => e.attrs?.figma_url },
    },
    component: {
      label: 'Component',
      plural: 'Components',
      display: { title: 'title', subtitle: (e) => e.attrs?.derived_from },
    },
    token: {
      label: 'Token',
      plural: 'Tokens',
      display: { title: 'title', subtitle: (e) => e.attrs?.value },
    },
    asset: {
      label: 'Asset',
      plural: 'Assets',
      display: { title: 'title', badge: (e) => e.attrs?.kind },
    },
    brief: {
      label: 'Brief',
      plural: 'Briefs',
      display: { title: 'title' },
    },
  },
  views: [
    { id: 'gallery', label: 'Assets', kind: 'gallery', type: 'asset', mediaField: 'blob_ref' },
    { id: 'library', label: 'Component Library', kind: 'table', type: 'component', columns: [
      { key: 'title', label: 'Component' },
      { key: 'derived_from', label: 'Derived From' },
    ] },
    { id: 'files', label: 'Design Files', kind: 'list', type: 'design_file' },
  ],
};

export const MODULE_MANIFESTS = {
  learning: LEARNING_MANIFEST,
  tasks: TASKS_MANIFEST,
  coding: CODING_MANIFEST,
  trading: TRADING_MANIFEST,
  social: SOCIAL_MANIFEST,
  marketing: MARKETING_MANIFEST,
  design: DESIGN_MANIFEST,
};

export function getManifest(id) {
  return MODULE_MANIFESTS[id];
}
