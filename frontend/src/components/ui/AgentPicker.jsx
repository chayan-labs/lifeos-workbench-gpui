import React, { useEffect, useState } from 'react';
import { Bot } from 'lucide-react';
import { apiCall } from '../../lib/api';
import { AGENT_CHANGED_EVENT, SELECTED_AGENT_KEY, SELECTED_MODEL_KEY } from '../../lib/ai';

// The on-the-go agent/model switcher. Mountable inside ANY AI surface
// (AI Console, Agent Harness, Knowledge Atlas, ...): it lists the agent
// CLIs the backend detected on PATH (GET /api/agents - keyless, they use
// their own logins) plus each CLI's suggested models, persists the choice
// to localStorage, and broadcasts AGENT_CHANGED_EVENT so every mounted
// picker stays in sync. All /api/llm call sites read the same keys via
// lib/ai.js llmSelection(), so switching here changes AI everywhere at once.

export default function AgentPicker({ compact = false, className = '' }) {
  const [agents, setAgents] = useState([]);
  const [backendDefault, setBackendDefault] = useState(null);
  const [agent, setAgent] = useState(localStorage.getItem(SELECTED_AGENT_KEY) || '');
  const [model, setModel] = useState(localStorage.getItem(SELECTED_MODEL_KEY) || '');
  const [offline, setOffline] = useState(false);

  useEffect(() => {
    apiCall('GET', '/api/agents').then(({ ok, data, offline: off }) => {
      if (off) { setOffline(true); return; }
      if (ok && data) {
        setAgents(data.agents || []);
        setBackendDefault(data.default || null);
      }
    });
    const sync = () => {
      setAgent(localStorage.getItem(SELECTED_AGENT_KEY) || '');
      setModel(localStorage.getItem(SELECTED_MODEL_KEY) || '');
    };
    window.addEventListener(AGENT_CHANGED_EVENT, sync);
    return () => window.removeEventListener(AGENT_CHANGED_EVENT, sync);
  }, []);

  const persist = (key, value) => {
    if (value) localStorage.setItem(key, value);
    else localStorage.removeItem(key);
    window.dispatchEvent(new CustomEvent(AGENT_CHANGED_EVENT));
  };

  const pickAgent = (id) => {
    setAgent(id);
    persist(SELECTED_AGENT_KEY, id);
    // A model string is agent-specific; switching agents resets it.
    setModel('');
    persist(SELECTED_MODEL_KEY, '');
  };

  const pickModel = (m) => {
    setModel(m);
    persist(SELECTED_MODEL_KEY, m);
  };

  const active = agents.find((a) => a.id === (agent || backendDefault));
  const suggestions = active?.models || [];

  if (offline) {
    return (
      <span className={`text-[10px] text-neo-text-muted font-mono ${className}`}>
        agents: backend offline
      </span>
    );
  }

  return (
    <div className={`flex items-center gap-1.5 ${className}`}>
      {!compact && <Bot size={13} className="text-neo-text-muted shrink-0" />}
      <select
        value={agent}
        onChange={(e) => pickAgent(e.target.value)}
        className="neo-border bg-neo-surface text-[11px] font-mono py-1 px-1.5 max-w-[140px]"
        title="Which local agent CLI answers AI requests"
      >
        <option value="">
          {backendDefault ? `auto (${backendDefault})` : 'auto'}
        </option>
        {agents.map((a) => (
          <option key={a.id} value={a.id}>
            {a.label}{a.verified ? '' : ' (unverified)'}
          </option>
        ))}
      </select>
      <input
        list="lifeos-agent-models"
        value={model}
        onChange={(e) => pickModel(e.target.value)}
        placeholder="model (default)"
        className="neo-border bg-neo-surface text-[11px] font-mono py-1 px-1.5 w-[130px]"
        title="Model override - pick a suggestion or type any model the CLI accepts"
      />
      <datalist id="lifeos-agent-models">
        {suggestions.map((m) => <option key={m} value={m} />)}
      </datalist>
    </div>
  );
}
