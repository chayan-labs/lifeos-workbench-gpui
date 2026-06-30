import React, { useState, useEffect } from 'react';
import {
  Boxes, RefreshCw, Cpu, Layers, ShieldAlert, Check, Terminal,
  Zap, Brain, Wrench, GitMerge, Crown, CircleDot, Power
} from 'lucide-react';
import { apiCall } from '../lib/api';
import { SELECTED_AGENT_KEY } from '../lib/ai';

/*
 * Open design harness composer.
 * - Detected agents come from GET /api/agents - the backend's real PATH scan
 *   (services/lifeos-api/src/agents.rs), not a hardcoded list.
 * - The chosen agent is persisted as the resolver AND as the agent every
 *   POST /api/llm call site uses (see lib/ai.js::selectedAgent).
 * - Layers the Life OS app harness on top of the chosen agent's base harness.
 * - Conflicts between the two layers are resolved by the USER'S chosen agent.
 * - The user picks which parts of the base/system harness to inherit.
 */

const HS = 'life_os_harness_state';

// Adapter-id -> display metadata not carried by the API response (vendor,
// "base" description). Falls back to a generic card for any future adapter
// added server-side without a frontend update.
const AGENT_META = {
  claude: { name: 'Claude Code', vendor: 'Anthropic', base: 'Hooks + Skills + Subagents' },
  gemini: { name: 'Gemini CLI', vendor: 'Google', base: 'MCP + tools' },
  codex: { name: 'OpenAI Codex', vendor: 'OpenAI', base: 'Tool registry + sandbox' },
  opencode: { name: 'OpenCode', vendor: 'OSS', base: 'Repo-map + tool calls' },
  hermes: { name: 'Hermes Agent', vendor: 'Local', base: 'Router + memvec recall' },
  openclaw: { name: 'OpenClaw', vendor: 'OSS', base: 'Agentic IDE harness' },
};
const metaFor = (id) => AGENT_META[id] || { name: id, vendor: 'Unknown', base: '-' };

// Parts of the base/system harness the user may choose to inherit.
const SYSTEM_PARTS = [
  { id: 'permissions', name: 'Permissions engine', icon: ShieldAlert, desc: 'allow/deny + prompt gating for tool calls' },
  { id: 'hooks', name: 'Hook system', icon: Zap, desc: 'PreToolUse / PostToolUse / Stop interception' },
  { id: 'memory', name: 'Memory & recall', icon: Brain, desc: 'FTS5 + vector recall across sessions' },
  { id: 'tools', name: 'Tool registry', icon: Wrench, desc: 'thin-HTTP tools + MCP multiplexer' },
  { id: 'router', name: 'Skill router', icon: Layers, desc: 'on-demand skill / rule / MCP discovery' },
  { id: 'subagents', name: 'Subagent orchestrator', icon: Boxes, desc: 'parallel split-role agents' },
];

// Conflicts the composer surfaces when the app harness overlaps the base harness.
const CONFLICTS = [
  { id: 'git-hook', area: 'PreToolUse hook (git)', base: 'broker-guard fails closed on order ops', app: 'Life OS adds commit-per-install gate' },
  { id: 'model-route', area: 'Model routing', base: 'agent default model', app: 'Life OS heavy/light lane policy' },
  { id: 'memory-ns', area: 'Memory namespace', base: 'agent global memory', app: 'workspace-scoped entities/events' },
];

const RESOLUTIONS = ['base', 'app', 'agent-merge'];
const RES_LABEL = { base: 'Keep base', app: 'Keep Life OS', 'agent-merge': 'Agent merges' };

const loadState = () => {
  try {
    const s = JSON.parse(localStorage.getItem(HS));
    if (s) return s;
  } catch {}
  return {
    userAgent: null, // resolved once GET /api/agents reports a default
    parts: Object.fromEntries(SYSTEM_PARTS.map((p) => [p.id, true])),
    resolutions: Object.fromEntries(CONFLICTS.map((c) => [c.id, 'agent-merge'])),
  };
};

export default function AgentHarness() {
  const [state, setState] = useState(loadState);
  const [scanning, setScanning] = useState(false);
  const [agents, setAgents] = useState([]);
  const [agentsState, setAgentsState] = useState('loading'); // 'loading' | 'ready' | 'offline'

  useEffect(() => {
    localStorage.setItem(HS, JSON.stringify(state));
    if (state.userAgent) localStorage.setItem(SELECTED_AGENT_KEY, state.userAgent);
  }, [state]);

  const rescan = () => {
    setScanning(true);
    apiCall('GET', '/api/agents').then(({ ok, data, offline }) => {
      setScanning(false);
      if (ok && !offline && data) {
        const detectedAgents = (data.agents || []).map((a) => ({
          id: a.id,
          name: metaFor(a.id).name,
          vendor: metaFor(a.id).vendor,
          base: metaFor(a.id).base,
          detected: true,
          version: a.verified ? 'verified' : 'unverified contract',
          path: a.path,
        }));
        setAgents(detectedAgents);
        setAgentsState('ready');
        // Keep the user's explicit choice; otherwise track the backend default.
        setState((s) => ({ ...s, userAgent: s.userAgent || data.default || detectedAgents[0]?.id || null }));
      } else {
        setAgentsState('offline');
      }
    });
  };

  useEffect(() => { rescan(); }, []);

  const detected = agents.filter((a) => a.detected);
  const userAgent = agents.find((a) => a.id === state.userAgent);

  const setUserAgent = (id) => setState((s) => ({ ...s, userAgent: id }));
  const togglePart = (id) => setState((s) => ({ ...s, parts: { ...s.parts, [id]: !s.parts[id] } }));
  const setResolution = (id, r) => setState((s) => ({ ...s, resolutions: { ...s.resolutions, [id]: r } }));

  return (
    <div className="flex flex-col gap-8 max-w-6xl">
      {/* Intro */}
      <div className="neo-surface neo-border-thick neo-shadow p-6">
        <h2 className="neo-title-md mb-2 flex items-center gap-2">
          <Boxes size={24} className="text-neo-blue" /> Open Harness Composer
        </h2>
        <p className="neo-body-md text-neo-text-muted">
          Life OS detects the coding agents on your host, layers its <strong>app harness</strong> on top of each agent's <strong>base harness</strong>, and lets <strong>your chosen agent</strong> resolve any conflicts. You decide which parts of the system harness to inherit - nothing is forced.
        </p>
      </div>

      {/* Detection */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-4">
        <div className="flex justify-between items-center border-b-2 border-neo-border pb-3">
          <h3 className="neo-title-md flex items-center gap-2"><Cpu size={18} /> Detected Agents ({detected.length})</h3>
          <button onClick={rescan} disabled={scanning} className="neo-btn bg-neo-yellow text-neo-text py-1 px-3 text-xs flex items-center gap-1.5">
            <RefreshCw size={12} className={scanning ? 'animate-spin' : ''} /> {scanning ? 'Scanning host…' : 'Re-scan host'}
          </button>
        </div>
        {agentsState === 'offline' && (
          <div className="px-3 py-2 bg-neo-red text-white text-xs font-bold neo-border">Backend unreachable - cannot scan PATH for agent CLIs.</div>
        )}
        {agentsState === 'ready' && detected.length === 0 && (
          <div className="px-3 py-2 bg-neo-surface-muted text-xs neo-border">No known agent CLI found on PATH.</div>
        )}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          {agents.map((a) => {
            const isUser = state.userAgent === a.id;
            return (
              <div
                key={a.id}
                className={`p-4 neo-border-thick flex flex-col gap-2 transition-all ${
                  a.detected ? 'bg-neo-surface' : 'bg-neo-surface-high opacity-60'
                } ${isUser ? 'neo-shadow border-neo-blue' : ''}`}
              >
                <div className="flex justify-between items-start">
                  <span className="font-extrabold uppercase text-sm text-neo-text" style={{ fontFamily: 'Montserrat, sans-serif' }}>{a.name}</span>
                  {a.detected
                    ? <span className="neo-tag bg-neo-mint text-neo-text"><CircleDot size={10} /> {a.version}</span>
                    : <span className="neo-tag text-neo-text-muted">not found</span>}
                </div>
                <p className="text-[11px] text-neo-text-muted">{a.vendor} · base: {a.base}</p>
                {a.detected && (
                  <button
                    onClick={() => setUserAgent(a.id)}
                    className={`mt-auto neo-btn py-1 px-2 text-[10px] flex items-center justify-center gap-1 ${
                      isUser ? 'bg-neo-blue text-white' : 'bg-neo-surface-high text-neo-text'
                    }`}
                  >
                    {isUser ? <><Crown size={11} /> Resolver agent</> : 'Make resolver'}
                  </button>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Layer stack */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-3">
        <h3 className="neo-title-md flex items-center gap-2"><Layers size={18} /> Harness Layer Stack</h3>
        <div className="flex flex-col gap-2">
          <div className="p-3 neo-border-thick bg-neo-yellow text-neo-text flex items-center gap-2">
            <Zap size={16} /> <span className="neo-label-sm">Life OS App Harness (top)</span>
            <span className="text-[11px] ml-auto">modules · self-extension · gates</span>
          </div>
          <div className="text-center text-neo-text-muted text-xs">▲ layered on ▲</div>
          <div className="p-3 neo-border-thick bg-neo-surface-high text-neo-text flex items-center gap-2">
            <Cpu size={16} /> <span className="neo-label-sm">{userAgent?.name || 'No'} Base Harness</span>
            <span className="text-[11px] ml-auto">{userAgent?.base}</span>
          </div>
          <div className="text-center text-neo-text-muted text-xs">resolves conflicts ▼</div>
          <div className="p-3 neo-border bg-neo-surface text-neo-text flex items-center gap-2">
            <Terminal size={16} /> <span className="neo-label-sm">Host (Mac OS · 127.0.0.1)</span>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
        {/* System parts toggles */}
        <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-3">
          <h3 className="neo-title-md flex items-center gap-2"><Power size={18} /> Inherit System Harness Parts</h3>
          <p className="text-xs text-neo-text-muted">Choose which parts of the base harness Life OS inherits.</p>
          {SYSTEM_PARTS.map((p) => {
            const on = state.parts[p.id];
            return (
              <button
                key={p.id}
                onClick={() => togglePart(p.id)}
                className={`flex items-center gap-3 p-3 neo-border text-left transition-all ${
                  on ? 'bg-neo-surface' : 'bg-neo-surface-high opacity-70'
                }`}
              >
                <p.icon size={16} className="shrink-0 text-neo-blue" />
                <div className="flex-1">
                  <div className="neo-label-sm text-neo-text">{p.name}</div>
                  <div className="text-[11px] text-neo-text-muted">{p.desc}</div>
                </div>
                <span className={`w-10 h-5 neo-border flex items-center px-0.5 transition-all ${on ? 'bg-neo-mint justify-end' : 'bg-neo-surface-high justify-start'}`}>
                  <span className="w-3.5 h-3.5 bg-neo-text block" />
                </span>
              </button>
            );
          })}
        </div>

        {/* Conflict resolution */}
        <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-3">
          <h3 className="neo-title-md flex items-center gap-2"><GitMerge size={18} /> Conflict Resolution</h3>
          <p className="text-xs text-neo-text-muted">
            Where the app harness overlaps the base harness, <strong>{userAgent?.name || 'your agent'}</strong> arbitrates per your choice.
          </p>
          {CONFLICTS.map((c) => (
            <div key={c.id} className="p-3 neo-border bg-neo-surface flex flex-col gap-2">
              <div className="flex items-center gap-2">
                <ShieldAlert size={14} className="text-neo-red shrink-0" />
                <span className="neo-label-sm text-neo-text">{c.area}</span>
              </div>
              <div className="text-[11px] text-neo-text-muted grid grid-cols-1 gap-0.5">
                <span><strong>base:</strong> {c.base}</span>
                <span><strong>app:</strong> {c.app}</span>
              </div>
              <div className="flex gap-1.5">
                {RESOLUTIONS.map((r) => {
                  const active = state.resolutions[c.id] === r;
                  return (
                    <button
                      key={r}
                      onClick={() => setResolution(c.id, r)}
                      className={`flex-1 py-1 text-[10px] font-bold uppercase neo-border transition-all flex items-center justify-center gap-1 ${
                        active ? 'bg-neo-blue text-white' : 'bg-neo-surface-high text-neo-text-muted'
                      }`}
                    >
                      {active && <Check size={10} />} {RES_LABEL[r]}
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
