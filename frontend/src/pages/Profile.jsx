import React, { useEffect, useState, useCallback } from 'react';
import {
  User, Building2, KeyRound, Save, Check, CreditCard, Gauge,
  ShieldCheck, Database, Boxes, FolderGit2, Crown, ShieldAlert, Lock, Wand2, Repeat
} from 'lucide-react';
import { LAYERS } from '../lib/capabilities';
import { getCapabilityMatrix } from '../lib/capabilityMatrix';
import { apiCall, WORKSPACE_ID_KEY } from '../lib/api';

const read = (k, fallback = '') => localStorage.getItem(k) || fallback;

const PLANS = [
  { id: 'free', name: 'Personal', price: 'Free', features: ['1 workspace', 'Local Mac harness', 'Unlimited modules'] },
  { id: 'pro', name: 'Pro', price: '$19/mo', features: ['5 workspaces', 'Cloud bot lane', 'Priority codegen'] },
  { id: 'team', name: 'Team', price: '$49/seat', features: ['Unlimited workspaces', 'Shared modules', 'SSO + audit log'] },
];

export default function Profile() {
  const [workspaceIdInput, setWorkspaceIdInput] = useState(read(WORKSPACE_ID_KEY, 'default-personal-workspace'));
  const [workspace, setWorkspace] = useState(null); // { id, name, plan } from GET /api/workspace
  const [workspaceName, setWorkspaceName] = useState('');
  const [activePlan, setActivePlan] = useState('free');
  const [me, setMe] = useState(null); // GET /api/me - { authenticated, id, email, name }
  const [usageRaw, setUsageRaw] = useState(null); // GET /api/metrics
  const [saved, setSaved] = useState(false);
  const [loading, setLoading] = useState(true);

  const loadAll = useCallback(async () => {
    setLoading(true);
    const [wsRes, meRes, metricsRes] = await Promise.all([
      apiCall('GET', '/api/workspace'),
      apiCall('GET', '/api/me'),
      apiCall('GET', '/api/metrics'),
    ]);
    if (wsRes.ok) {
      setWorkspace(wsRes.data);
      setWorkspaceName(wsRes.data.name);
      setActivePlan(wsRes.data.plan || 'free');
    }
    if (meRes.ok) setMe(meRes.data);
    if (metricsRes.ok) setUsageRaw(metricsRes.data);
    setLoading(false);
  }, []);

  useEffect(() => { loadAll(); }, [loadAll]);

  // Switching the workspace id changes the X-Workspace-Id header every
  // subsequent apiCall sends (lib/api.js reads it fresh from localStorage on
  // every request) - this is the P0 wiring this issue requires.
  const switchWorkspace = () => {
    localStorage.setItem(WORKSPACE_ID_KEY, workspaceIdInput.trim() || 'default-personal-workspace');
    loadAll();
  };

  const handleSave = async () => {
    const { ok, data } = await apiCall('PATCH', '/api/workspace', { name: workspaceName.trim(), plan: activePlan });
    if (ok) {
      setWorkspace(data);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    }
  };

  const name = me?.authenticated ? (me.name || me.email) : 'Demo user (no key_token)';
  const email = me?.authenticated ? me.email : read('life_os_user_email', 'chayan@life-os.dev');
  const initials = name.split(' ').map((p) => p[0]).join('').slice(0, 2).toUpperCase() || 'LO';

  const usage = usageRaw ? [
    { label: 'Entities stored', value: String(usageRaw.entities), max: '∞', icon: Database, pct: Math.min(100, usageRaw.entities) },
    { label: 'Modules touched', value: String(Object.keys(usageRaw.entities_by_module || {}).length), max: '∞', icon: Boxes, pct: 14 },
    { label: 'Events logged', value: String(usageRaw.events), max: '∞', icon: FolderGit2, pct: Math.min(100, usageRaw.events) },
    { label: 'Harness runs', value: String(usageRaw.harness_runs), max: '∞', icon: Gauge, pct: Math.min(100, usageRaw.harness_runs) },
  ] : [];

  return (
    <div className="flex flex-col gap-8 max-w-5xl">
      {/* Identity header */}
      <div className="neo-surface neo-border-thick neo-shadow p-6 flex flex-col sm:flex-row items-center gap-5">
        <div className="w-20 h-20 neo-border neo-shadow bg-neo-yellow flex items-center justify-center text-2xl font-extrabold text-neo-text shrink-0">
          {initials}
        </div>
        <div className="flex-1 text-center sm:text-left">
          <h2 className="neo-title-md text-neo-text">{name}</h2>
          <p className="neo-body-md text-neo-text-muted">{email}</p>
          <div className="flex flex-wrap gap-2 mt-2 justify-center sm:justify-start">
            <span className="neo-tag bg-neo-mint text-neo-text"><ShieldCheck size={12} /> Verified owner</span>
            <span className="neo-tag bg-neo-yellow text-neo-text"><Crown size={12} /> {PLANS.find((p) => p.id === activePlan)?.name} plan</span>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
        {/* Account settings */}
        <div className="lg:col-span-7 neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-4">
          <h3 className="neo-title-md flex items-center gap-2"><User size={18} /> Account Settings</h3>

          <label className="flex flex-col gap-1">
            <span className="neo-label-sm text-neo-text-muted">Display Name</span>
            <input className="neo-input opacity-60 cursor-not-allowed" value={name} disabled title="Set at registration via POST /api/register - no update-user route exists yet." />
          </label>

          <label className="flex flex-col gap-1">
            <span className="neo-label-sm text-neo-text-muted">Email</span>
            <input className="neo-input opacity-60 cursor-not-allowed" value={email} disabled />
          </label>

          <label className="flex flex-col gap-1">
            <span className="neo-label-sm text-neo-text-muted flex items-center gap-1"><Building2 size={12} /> Workspace Name</span>
            <input className="neo-input" value={workspaceName} onChange={(e) => setWorkspaceName(e.target.value)} />
          </label>

          <label className="flex flex-col gap-1">
            <span className="neo-label-sm text-neo-text-muted flex items-center gap-1"><KeyRound size={12} /> Tenant ID (X-Workspace-Id)</span>
            <div className="flex gap-2">
              <input
                className="neo-input font-mono text-xs flex-1"
                value={workspaceIdInput}
                onChange={(e) => setWorkspaceIdInput(e.target.value)}
              />
              <button onClick={switchWorkspace} className="neo-btn bg-neo-surface-high text-neo-text px-3 flex items-center gap-1.5 text-xs font-bold shrink-0">
                <Repeat size={13} /> Switch
              </button>
            </div>
            <span className="text-[10px] text-neo-text-muted">Resolved workspace: <code>{workspace?.id || '…'}</code> - changing this updates every request's <code>X-Workspace-Id</code> header.</span>
          </label>

          <button onClick={handleSave} className="neo-btn bg-neo-mint text-neo-text py-2 px-4 flex items-center justify-center gap-2 self-start">
            {saved ? <><Check size={16} /> Saved</> : <><Save size={16} /> Save Changes</>}
          </button>
        </div>

        {/* Usage */}
        <div className="lg:col-span-5 neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-4">
          <h3 className="neo-title-md flex items-center gap-2"><Gauge size={18} /> Usage</h3>
          {loading && <p className="text-xs text-neo-text-muted">Loading…</p>}
          {!loading && !usage.length && <p className="text-xs text-neo-text-muted">No metrics yet.</p>}
          {usage.map((u) => (
            <div key={u.label} className="flex flex-col gap-1">
              <div className="flex justify-between items-center neo-label-sm">
                <span className="flex items-center gap-1.5 text-neo-text-muted"><u.icon size={13} /> {u.label}</span>
                <span className="text-neo-text">{u.value} <span className="text-neo-text-muted">/ {u.max}</span></span>
              </div>
              <div className="h-2.5 neo-border bg-neo-surface-high">
                <div className="h-full bg-neo-blue" style={{ width: `${u.pct}%` }} />
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Plans */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-4">
        <h3 className="neo-title-md flex items-center gap-2"><CreditCard size={18} /> Subscription Plan</h3>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {PLANS.map((p) => {
            const isActive = activePlan === p.id;
            return (
              <button
                key={p.id}
                onClick={() => setActivePlan(p.id)}
                className={`text-left p-4 neo-border-thick transition-all flex flex-col gap-2 ${
                  isActive ? 'bg-neo-yellow text-neo-text neo-shadow' : 'bg-neo-surface hover:bg-neo-surface-high'
                }`}
              >
                <div className="flex justify-between items-baseline">
                  <span className="neo-title-md text-base">{p.name}</span>
                  <span className="neo-label-sm">{p.price}</span>
                </div>
                <ul className="flex flex-col gap-1">
                  {p.features.map((f) => (
                    <li key={f} className="text-xs flex items-center gap-1.5 text-neo-text-muted">
                      <Check size={12} className="text-neo-blue shrink-0" /> {f}
                    </li>
                  ))}
                </ul>
                {isActive && <span className="neo-tag bg-neo-surface text-neo-text mt-1 self-start">Current</span>}
              </button>
            );
          })}
        </div>
        <p className="text-xs text-neo-text-muted">Personal use runs entirely on your trusted Mac. Upgrading is a deployment swap, not a rewrite - multi-tenant from day one.</p>
      </div>

      {/* AI Guardrails - what AI can and cannot touch */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-3">
        <h3 className="neo-title-md flex items-center gap-2"><ShieldAlert size={18} /> AI Guardrails</h3>
        <p className="text-xs text-neo-text-muted">
          Life OS is self-evolving: AI can reshape any non-gated layer. <strong>Gated</strong> layers are human-only (no AI read/write); <strong>core</strong> layers AI can modify but never delete. Every change is reversible via VCS time-travel.
        </p>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
          {LAYERS.map((l) => (
            <div key={l.id} className="p-3 neo-border bg-neo-surface flex items-center justify-between gap-2">
              <div className="min-w-0">
                <div className="text-sm font-bold text-neo-text truncate">{l.label}</div>
                <div className="text-[10px] text-neo-text-muted font-mono">{l.group}</div>
              </div>
              <div className="flex items-center gap-1 shrink-0">
                {l.gated ? (
                  <span className="neo-tag bg-neo-red text-white text-[9px]"><Lock size={9} /> gated</span>
                ) : (
                  <>
                    {l.aiCanModify && <span className="neo-tag bg-neo-mint text-neo-text text-[9px]"><Wand2 size={9} /> modify</span>}
                    {l.core
                      ? <span className="neo-tag bg-neo-yellow text-neo-text text-[9px]">core</span>
                      : l.aiCanDelete && <span className="neo-tag text-[9px]">delete</span>}
                  </>
                )}
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Agent Control Plane capability matrix - the canonical, read-only
          {allowed|gated|forbidden} view (docs/AGENT-CONTROL.md §5). There is
          no edit control anywhere on this page for this section: the matrix
          is derived from agentActions.js + capabilities.js and cannot widen
          the agent's own reach from the UI. */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-3">
        <h3 className="neo-title-md flex items-center gap-2"><ShieldCheck size={18} /> Agent Control Plane (read-only)</h3>
        <p className="text-xs text-neo-text-muted">
          Exactly what the in-app agent can and cannot do, across both typed action tools and app layers.
          <strong> Forbidden</strong> means no tool/access exists at all - not a check that could be bypassed.
          <strong> Gated</strong> means a human must approve before it runs. This view is read-only; it cannot be edited from the app.
        </p>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
          {getCapabilityMatrix().map((row) => (
            <div key={`${row.kind}-${row.id}`} className="p-3 neo-border bg-neo-surface flex items-center justify-between gap-2">
              <div className="min-w-0">
                <div className="text-sm font-bold text-neo-text truncate font-mono">{row.label}</div>
                <div className="text-[10px] text-neo-text-muted">{row.kind}</div>
              </div>
              {row.classification === 'forbidden' && (
                <span className="neo-tag bg-neo-red text-white text-[9px] shrink-0"><Lock size={9} /> forbidden</span>
              )}
              {row.classification === 'gated' && (
                <span className="neo-tag bg-neo-yellow text-neo-text text-[9px] shrink-0">gated</span>
              )}
              {row.classification === 'allowed' && (
                <span className="neo-tag bg-neo-mint text-neo-text text-[9px] shrink-0">allowed</span>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
