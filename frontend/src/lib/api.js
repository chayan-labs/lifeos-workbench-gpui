// Central API client + route registry for Life OS.
// Every backend route the system exposes (live or planned) is declared here so
// the frontend can both call it AND surface it in the API Explorer. The base URL
// is configurable via VITE_API_URL; it falls back to the local trusted host.

export const API_BASE =
  (import.meta.env && import.meta.env.VITE_API_URL) || 'http://127.0.0.1:8080';

// Self-hosted Nango Connect UI (infra/nango/docker-compose.yml CONNECT_UI_PORT).
export const NANGO_CONNECT_UI_URL =
  (import.meta.env && import.meta.env.VITE_NANGO_CONNECT_URL) || 'http://localhost:3009';

// Canonical localStorage keys for tenant + soft-auth state. See FRONTEND.md §1.
export const WORKSPACE_ID_KEY = 'life_os_workspace_id';
export const KEY_TOKEN_KEY = 'life_os_key_token';
const DEFAULT_WORKSPACE_ID = 'default-personal-workspace';

// status: 'live'    -> implemented in lifeos-api (services/lifeos-api/src/)
//         'queued'  -> implemented: enqueues a job, returns 202 (service drains later)
//         'planned' -> route exists but honestly returns 501 until its phase ships
export const API_ROUTES = [
  {
    service: 'lifeos-api',
    method: 'GET',
    path: '/api/health',
    status: 'live',
    summary: 'Liveness probe for the canonical API.',
    sample: null,
  },
  {
    service: 'lifeos-api',
    method: 'POST',
    path: '/api/entity',
    status: 'live',
    summary: 'Create an entity (generic workspace + module + type + attrs row).',
    sample: { module: 'tasks', type: 'task', title: 'Ship overhaul', attrs: { priority: 'high' } },
  },
  {
    service: 'lifeos-api',
    method: 'GET',
    path: '/api/entity',
    status: 'live',
    summary: 'Query / list entities by workspace + module + type + status.',
    sample: null,
  },
  {
    service: 'lifeos-api',
    method: 'GET',
    path: '/api/entity/:id',
    status: 'live',
    summary: 'Fetch a single entity by id (workspace-scoped).',
    sample: null,
  },
  {
    service: 'lifeos-api',
    method: 'PATCH',
    path: '/api/entity/:id',
    status: 'live',
    summary: 'Update an entity (lifecycle / attrs). Emits an entity.updated event.',
    sample: { status: 'completed' },
  },
  {
    service: 'lifeos-api',
    method: 'POST',
    path: '/api/edge',
    status: 'live',
    summary: 'Create a graph relation between entities (or to an external ref).',
    sample: { src_id: 'ent_a', dst_id: 'ent_b', rel: 'depends_on' },
  },
  {
    service: 'lifeos-api',
    method: 'GET',
    path: '/api/edge',
    status: 'live',
    summary: 'List edges by src / dst / relation.',
    sample: null,
  },
  {
    service: 'lifeos-api',
    method: 'POST',
    path: '/api/event',
    status: 'live',
    summary: 'Append a domain / harness event. Append-only: no update or delete route exists.',
    sample: { type: 'study.review', actor: 'user' },
  },
  {
    service: 'lifeos-api',
    method: 'GET',
    path: '/api/event',
    status: 'live',
    summary: 'Read the append-only event / run log.',
    sample: null,
  },
  {
    service: 'lifeos-api',
    method: 'GET',
    path: '/api/jobs',
    status: 'live',
    summary: 'Read the job queue lifeos-drain claims from.',
    sample: null,
  },
  {
    service: 'lifeos-api',
    method: 'GET',
    path: '/api/metrics',
    status: 'live',
    summary: 'SQL aggregation over events + entities for dashboards.',
    sample: null,
  },
  {
    service: 'lifeos-api',
    method: 'POST',
    path: '/api/module-request',
    status: 'live',
    summary: 'Record a self-extension request, log it, and enqueue a module_build job.',
    sample: { prompt: 'Add a habit-tracking module' },
  },
  {
    service: 'lifeos-api',
    method: 'POST',
    path: '/api/register',
    status: 'live',
    summary: 'Register a new workspace (tenant) + user and mint a key_token.',
    sample: { email: 'me@example.com', name: 'Me', workspace_name: 'My Life OS' },
  },
  {
    service: 'lifeos-api',
    method: 'GET',
    path: '/api/me',
    status: 'live',
    summary: 'The authenticated user from the bearer token claims, if any.',
    sample: null,
  },
  {
    service: 'lifeos-api',
    method: 'GET',
    path: '/api/workspace',
    status: 'live',
    summary: 'The resolved workspace row (name/plan).',
    sample: null,
  },
  {
    service: 'lifeos-api',
    method: 'PATCH',
    path: '/api/workspace',
    status: 'live',
    summary: 'Rename the workspace or change its plan.',
    sample: { name: 'My Life OS' },
  },
  {
    service: 'lifeos-api',
    method: 'GET',
    path: '/api/agents',
    status: 'live',
    summary: 'List local agent CLIs detected on PATH (the /api/llm engines) + the default.',
    sample: null,
  },
  {
    service: 'lifeos-api',
    method: 'POST',
    path: '/api/llm',
    status: 'live',
    summary: 'Route a prompt to a local agent CLI (Claude Code / Gemini / ...) and return { text }.',
    sample: { system: 'You are a study assistant.', prompt: 'Explain CAP theorem.' },
  },
  {
    service: 'lifeos-api',
    method: 'GET',
    path: '/api/connections',
    status: 'live',
    summary: 'List owned-credential connections (provider, handle, status). Never returns a token.',
    sample: null,
  },
  {
    service: 'lifeos-api',
    method: 'POST',
    path: '/api/connections/session',
    status: 'live',
    summary: "Mint a Nango Connect session token to run a provider's OAuth dance.",
    sample: { provider: 'google-mail' },
  },
  {
    service: 'lifeos-api',
    method: 'POST',
    path: '/api/connections/complete',
    status: 'live',
    summary: 'Record a connection after the Nango Connect UI reports success.',
    sample: { connection_id: 'con_x_091', provider: 'google-mail' },
  },
  {
    service: 'lifeos-api',
    method: 'DELETE',
    path: '/api/connections/:id',
    status: 'live',
    summary: 'Revoke a connection with Nango and mark it disconnected.',
    sample: null,
  },
  {
    service: 'lifeos-ingest',
    method: 'POST',
    path: '/api/ingest',
    status: 'queued',
    summary: 'Enqueue media ingest (transcribe / caption / parse into segments). Returns 202.',
    sample: { uri: 's3://lifeos-vault/demo-reel.mp4', kind: 'video' },
  },
  {
    service: 'lifeos-pipelines',
    method: 'POST',
    path: '/api/pipeline/run',
    status: 'queued',
    summary: 'Enqueue a Life OS Action / pipeline DAG run. Returns 202.',
    sample: { pipeline: 'social-draft', input: {} },
  },
  {
    service: 'lifeos-vcs',
    method: 'GET',
    path: '/api/vcs/history',
    status: 'planned',
    summary: 'Content-addressed version history (501 until the VCS phase).',
    sample: null,
  },
  {
    service: 'lifeos-vcs',
    method: 'POST',
    path: '/api/vcs/commit',
    status: 'planned',
    summary: 'Commit a new content-addressed version (501 until the VCS phase).',
    sample: { path: 'modules/learning/module.js', blake3: 'b3:…' },
  },
  {
    service: 'broker-guard',
    method: 'GET',
    path: '/api/broker/positions',
    status: 'planned',
    summary: 'Read-only broker positions (501 until the trading phase). Order routes never exist.',
    sample: null,
  },
];

// Tenant + soft-auth headers attached to every request, mirroring core/db.js's
// header shape (X-Workspace-Id always; Authorization only once a key_token
// exists). Without these the backend silently falls back to the default
// workspace, masking tenant bugs - see FRONTEND.md §1.
function authHeaders(json) {
  const headers = {};
  if (json) headers['Content-Type'] = 'application/json';
  headers['X-Workspace-Id'] = localStorage.getItem(WORKSPACE_ID_KEY) || DEFAULT_WORKSPACE_ID;
  const keyToken = localStorage.getItem(KEY_TOKEN_KEY);
  if (keyToken) headers['Authorization'] = `Bearer ${keyToken}`;
  return headers;
}

// Thin fetch wrapper. Returns { ok, data, error, offline }.
export async function apiCall(method, path, body) {
  try {
    const res = await fetch(`${API_BASE}${path}`, {
      method,
      headers: authHeaders(Boolean(body)),
      body: body ? JSON.stringify(body) : undefined,
    });
    const text = await res.text();
    let data = null;
    try { data = text ? JSON.parse(text) : null; } catch { data = text; }
    return { ok: res.ok, status: res.status, data, error: res.ok ? null : (data?.error || res.statusText), offline: false };
  } catch (e) {
    return { ok: false, status: 0, data: null, error: e.message, offline: true };
  }
}

// Ping a single route's health-style endpoint. Used by the API Explorer.
export async function pingRoute(route) {
  // Only GET routes with no required body are safely pingable; others report 'unknown'.
  if (route.method !== 'GET') return { reachable: null };
  const { ok, offline } = await apiCall('GET', route.path);
  return { reachable: offline ? false : ok };
}
