// Agent Control Plane: the closed, typed action-tool registry the in-app
// agent must go through to actuate anything (docs/AGENT-CONTROL.md §1-2).
// Hard rule: the four protected domains (VCS rewrite, security/gating
// config, OAuth/connection creation, secret reads) have NO entry here at
// all - not "forbidden", *absent* - so there is no tool object an agent
// could even reference for them. Anything not absent is allowed or gated.
import { apiCall } from './api';

// Every name that would actuate a protected domain. Listed explicitly (not
// inferred from ACTION_TOOLS) so adding a new legitimate tool can never
// accidentally collide with - or be confused for - a protected one.
export const PROTECTED_TOOLS = Object.freeze([
  'vcs.rewrite',
  'vcs.branchForce',
  'vcs.gc',
  'vcs.deleteVersion',
  'security.configure',
  'security.setGating',
  'connection.create',
  'connection.revoke',
  'secret.read',
  'secret.write',
]);

// tool -> { classification: 'allowed' | 'gated', run(args) -> Promise<apiCall result> }
// `allowed` = reversible + internal, auto-applies. `gated` = outward/
// irreversible, requires human approval before `run` is ever invoked (the
// approval workflow itself is a separate issue; this registry only refuses
// to auto-execute gated tools).
export const ACTION_TOOLS = Object.freeze({
  'entity.create': {
    classification: 'allowed',
    run: (args) => apiCall('POST', '/api/entity', args),
  },
  'entity.update': {
    classification: 'allowed',
    run: (args) => apiCall('PATCH', `/api/entity/${args.id}`, args.patch),
  },
  'edge.create': {
    classification: 'allowed',
    run: (args) => apiCall('POST', '/api/edge', args),
  },
  // Drafting content for outward channels (social/email/etc.) is gated even
  // though the draft entity itself is internal - docs/AGENT-CONTROL.md §3
  // calls this out explicitly ("draft.create (gated, since publishing is
  // outward)"), so approval happens before the draft is even written.
  'draft.create': {
    classification: 'gated',
    run: (args) => apiCall('POST', '/api/entity', { ...args, status: args.status || 'drafted' }),
  },
  'view.configure': {
    classification: 'allowed',
    // No backend route yet (saved views are a later epic) - persists to
    // localStorage so the tool is real today rather than a stub that lies
    // about succeeding.
    run: (args) => {
      localStorage.setItem(`life_os_view_${args.id}`, JSON.stringify(args.config));
      return Promise.resolve({ ok: true, data: args, error: null, offline: false });
    },
  },
  'dashboard.arrange': {
    classification: 'allowed',
    run: (args) => {
      localStorage.setItem('life_os_dashboard_layout', JSON.stringify(args.layout));
      return Promise.resolve({ ok: true, data: args, error: null, offline: false });
    },
  },
  'navigate': {
    classification: 'allowed',
    // The caller (CommandBar/AIConsole) supplies the actual router navigate
    // function; this registry only validates the action shape exists.
    run: (args) => Promise.resolve({ ok: true, data: args, error: null, offline: false }),
  },
  // Pipelines (#92+) aren't built yet; their queued route already exists -
  // classified gated since a pipeline's stages can themselves reach gated
  // tools, and the registry can't know in advance which.
  'pipeline.run': {
    classification: 'gated',
    run: (args) => apiCall('POST', '/api/pipeline/run', args),
  },
  'module.requestBuild': {
    classification: 'allowed',
    run: (args) => apiCall('POST', '/api/module-request', args),
  },
  'search': {
    classification: 'allowed',
    run: (args) => apiCall('GET', `/api/search?${new URLSearchParams(args).toString()}`),
  },
});

export function classifyAction(tool) {
  if (PROTECTED_TOOLS.includes(tool)) return 'forbidden';
  if (ACTION_TOOLS[tool]) return ACTION_TOOLS[tool].classification;
  return 'forbidden'; // unknown tool = no tool = forbidden by default (closed set)
}

async function logDenied(tool, args) {
  await apiCall('POST', '/api/event', {
    type: 'action.denied',
    actor: 'agent',
    attrs: { tool, args },
  });
}

async function logApplied(tool, args, result) {
  await apiCall('POST', '/api/event', {
    type: 'action.applied',
    actor: 'agent',
    entity_id: result?.data?.id,
    attrs: { tool, args },
  });
}

// The single chokepoint every agent action must pass through. Returns
// { status: 'forbidden' | 'pending_approval' | 'applied' | 'failed', ... }
// - never executes a gated tool without explicit `approved: true`, and never
// even looks up a protected tool name beyond classifying it as forbidden.
export async function executeAction({ tool, args }, { approved = false } = {}) {
  const classification = classifyAction(tool);

  if (classification === 'forbidden') {
    await logDenied(tool, args);
    return { status: 'forbidden', tool, reason: `'${tool}' has no action tool - this domain is hard-denied (docs/AGENT-CONTROL.md §1).` };
  }

  if (classification === 'gated' && !approved) {
    return { status: 'pending_approval', tool, reason: `'${tool}' is gated - outward/irreversible actions require human approval before they run.` };
  }

  const result = await ACTION_TOOLS[tool].run(args);
  if (!result.ok) {
    return { status: 'failed', tool, error: result.error };
  }
  await logApplied(tool, args, result);
  return { status: 'applied', tool, data: result.data };
}
