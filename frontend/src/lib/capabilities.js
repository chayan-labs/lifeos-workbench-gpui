// Guardrail registry - the single source of truth for what AI may touch.
//
// Life OS is a self-evolving app: AI is available at every layer so the user
// can reshape almost anything. The safety model is enforced here:
//   - aiCanModify : AI may change this layer (within bounds) on user request.
//   - aiCanDelete : AI may remove user-created items in this layer.
//   - core        : core functionality. AI can MODIFY core within bounds but
//                   may NEVER delete it or break its baseline behavior.
//   - gated       : fully off-limits to AI (read AND write). Only the human
//                   acts here. The four gated systems were chosen by the user:
//                   VCS+history, the security/token vault, this registry
//                   itself (no self-granting privileges), and tenant/billing.
//
// THIS FILE IS ITSELF GATED. AI must not edit capabilities.js - doing so would
// let it rewrite its own permissions. Changes here are human-only, by design.

export const LAYERS = [
  // --- Mutable, non-core: AI has the most freedom here ---
  { id: 'theme', label: 'Theme & appearance', group: 'Appearance', aiCanRead: true, aiCanModify: true, aiCanDelete: false, core: false, gated: false },
  { id: 'dashboard', label: 'Dashboard widgets', group: 'Workspace', aiCanRead: true, aiCanModify: true, aiCanDelete: true, core: false, gated: false },
  { id: 'atlas-domains', label: 'Knowledge domains & topics', group: 'Knowledge', aiCanRead: true, aiCanModify: true, aiCanDelete: true, core: false, gated: false },
  { id: 'atlas-notes', label: 'Notes, papers & roadmaps', group: 'Knowledge', aiCanRead: true, aiCanModify: true, aiCanDelete: true, core: false, gated: false },
  { id: 'projects', label: 'Projects (recommended/accepted)', group: 'Knowledge', aiCanRead: true, aiCanModify: true, aiCanDelete: true, core: false, gated: false },
  { id: 'modules-content', label: 'Module data & entities', group: 'Modules', aiCanRead: true, aiCanModify: true, aiCanDelete: true, core: false, gated: false },
  { id: 'harness', label: 'Harness composition', group: 'Build', aiCanRead: true, aiCanModify: true, aiCanDelete: false, core: false, gated: false },

  // --- Core functionality: AI may modify within bounds, NEVER delete ---
  { id: 'navigation', label: 'Navigation & routing', group: 'Shell', aiCanRead: true, aiCanModify: true, aiCanDelete: false, core: true, gated: false },
  { id: 'module-system', label: 'Module system (declarative engine)', group: 'Modules', aiCanRead: true, aiCanModify: true, aiCanDelete: false, core: true, gated: false },
  { id: 'entities-schema', label: 'Generic entities schema', group: 'Data', aiCanRead: true, aiCanModify: true, aiCanDelete: false, core: true, gated: false },
  { id: 'knowledge-feature', label: 'Knowledge Atlas feature', group: 'Knowledge', aiCanRead: true, aiCanModify: true, aiCanDelete: false, core: true, gated: false },
  { id: 'storage-feature', label: 'Storage & repository feature', group: 'Data', aiCanRead: true, aiCanModify: true, aiCanDelete: false, core: true, gated: false },

  // --- Gated: AI has NO access (read or write). Human-only. ---
  { id: 'vcs', label: 'Version control & time-travel', group: 'Safety', aiCanRead: false, aiCanModify: false, aiCanDelete: false, core: true, gated: true },
  { id: 'security-vault', label: 'Security & token vault (Nango, broker-guard)', group: 'Safety', aiCanRead: false, aiCanModify: false, aiCanDelete: false, core: true, gated: true },
  { id: 'guardrails', label: 'Guardrail registry (this file)', group: 'Safety', aiCanRead: false, aiCanModify: false, aiCanDelete: false, core: true, gated: true },
  { id: 'tenant-billing', label: 'Tenant identity & billing', group: 'Safety', aiCanRead: false, aiCanModify: false, aiCanDelete: false, core: true, gated: true },
];

export const getLayer = (id) => LAYERS.find((l) => l.id === id) || null;

// Central guardrail check. action ∈ 'read' | 'modify' | 'delete'.
// Returns { allowed, reason }.
export function canAI(action, layerId) {
  const layer = getLayer(layerId);
  if (!layer) return { allowed: false, reason: `Unknown layer "${layerId}".` };
  if (layer.gated) {
    return { allowed: false, reason: `"${layer.label}" is gated from AI. Only you can change it.` };
  }
  if (action === 'read') {
    return layer.aiCanRead ? { allowed: true } : { allowed: false, reason: `AI cannot read "${layer.label}".` };
  }
  if (action === 'modify') {
    return layer.aiCanModify ? { allowed: true } : { allowed: false, reason: `AI cannot modify "${layer.label}".` };
  }
  if (action === 'delete') {
    if (layer.core) return { allowed: false, reason: `"${layer.label}" is core functionality - AI may modify it but never delete it.` };
    return layer.aiCanDelete ? { allowed: true } : { allowed: false, reason: `AI cannot delete "${layer.label}".` };
  }
  return { allowed: false, reason: 'Unknown action.' };
}

// Naive intent -> layer router for the AI Console (showcase). Maps free text to
// the layers it would touch, so the console can show scope + run guardrail checks.
const KEYWORDS = {
  theme: ['theme', 'color', 'dark', 'light', 'font', 'appearance', 'style'],
  dashboard: ['dashboard', 'widget', 'home', 'overview'],
  'atlas-domains': ['domain', 'topic', 'atlas', 'subject', 'curriculum'],
  'atlas-notes': ['note', 'paper', 'roadmap', 'summary', 'summarize'],
  projects: ['project', 'build idea', 'recommend project'],
  'modules-content': ['module', 'task', 'kanban', 'entity', 'record', 'social', 'design'],
  harness: ['harness', 'agent', 'resolver', 'codex', 'claude code', 'hermes'],
  navigation: ['nav', 'menu', 'sidebar', 'route', 'page', 'tab'],
  vcs: ['vcs', 'commit', 'history', 'version', 'time travel', 'snapshot', 'rollback', 'restore'],
  'security-vault': ['token', 'secret', 'credential', 'oauth', 'nango', 'api key', 'broker'],
  guardrails: ['guardrail', 'permission', 'capability', 'allow ai', 'gate'],
  'tenant-billing': ['billing', 'plan', 'subscription', 'tenant', 'workspace id', 'payment'],
};

export function routeIntent(text) {
  const t = String(text || '').toLowerCase();
  const hits = LAYERS.filter((l) => (KEYWORDS[l.id] || []).some((k) => t.includes(k)));
  return hits.length ? hits : [getLayer('modules-content')]; // default target
}
