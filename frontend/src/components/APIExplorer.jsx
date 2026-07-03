import React, { useState } from 'react';
import { Network, Radio, CheckCircle2, XCircle, CircleDashed, Send } from 'lucide-react';
import { API_ROUTES, API_BASE, apiCall, pingRoute } from '../lib/api';

// Surfaces the entire system API in the frontend (and vice-versa): every route
// the platform exposes - live or planned - is listed, pingable, and callable
// with its sample payload. This gives full visibility into "how to do anything"
// even before a given backend service exists.

const METHOD_COLOR = { GET: 'bg-neo-mint', POST: 'bg-neo-yellow', DELETE: 'bg-neo-red text-white' };

export default function APIExplorer() {
  const [health, setHealth] = useState({}); // path -> true/false/null
  const [pinging, setPinging] = useState(false);
  const [tried, setTried] = useState({}); // path -> response string

  const pingAll = async () => {
    setPinging(true);
    const next = {};
    for (const r of API_ROUTES) {
      const { reachable } = await pingRoute(r);
      next[r.path + r.method] = reachable;
    }
    setHealth(next);
    setPinging(false);
  };

  const tryRoute = async (r) => {
    const { ok, data, error, offline } = await apiCall(r.method, r.path, r.sample || undefined);
    const body = offline ? 'offline / not reachable' : (typeof data === 'string' ? data : JSON.stringify(data));
    setTried((t) => ({ ...t, [r.path + r.method]: ok ? `200 ${body}`.slice(0, 200) : `${error || 'error'}`.slice(0, 200) }));
  };

  const services = [...new Set(API_ROUTES.map((r) => r.service))];

  return (
    <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface flex flex-col gap-4">
      <div className="flex justify-between items-center border-b-2 border-neo-border pb-3 flex-wrap gap-2">
        <h3 className="neo-title-md flex items-center gap-2"><Network size={18} className="text-neo-blue" /> API Explorer</h3>
        <button onClick={pingAll} disabled={pinging} className="neo-btn bg-neo-yellow text-neo-text py-1 px-3 text-xs flex items-center gap-1.5">
          <Radio size={12} className={pinging ? 'animate-pulse' : ''} /> {pinging ? 'Pinging…' : 'Ping all routes'}
        </button>
      </div>
      <p className="text-xs text-neo-text-muted">
        Every route the system exposes, surfaced here. <code className="text-neo-blue">{API_BASE}</code> · <span className="text-neo-mint font-bold">live</span> = implemented in <code>lifeos-api</code>; <span className="text-neo-text-muted font-bold">planned</span> = the frontend expects it, backend pending.
      </p>

      {services.map((svc) => (
        <div key={svc} className="flex flex-col gap-2">
          <div className="neo-label-sm text-neo-text-muted text-[10px] mt-1">{svc}</div>
          {API_ROUTES.filter((r) => r.service === svc).map((r) => {
            const key = r.path + r.method;
            const reach = health[key];
            return (
              <div key={key} className="p-3 neo-border bg-neo-bg flex flex-col gap-1.5">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className={`neo-tag text-[9px] ${METHOD_COLOR[r.method] || 'bg-neo-surface-high'}`}>{r.method}</span>
                  <code className="text-xs font-mono font-bold text-neo-text">{r.path}</code>
                  <span className={`neo-tag text-[9px] ${r.status === 'live' ? 'bg-neo-mint text-neo-text' : 'bg-neo-surface-high text-neo-text-muted'}`}>{r.status}</span>
                  {reach === true && <CheckCircle2 size={14} className="text-neo-mint" />}
                  {reach === false && <XCircle size={14} className="text-neo-red" />}
                  {reach === null && <CircleDashed size={14} className="text-neo-text-muted" />}
                  <button onClick={() => tryRoute(r)} className="neo-btn bg-neo-surface text-neo-text py-0.5 px-2 text-[10px] flex items-center gap-1 ml-auto">
                    <Send size={10} /> Try
                  </button>
                </div>
                <p className="text-[11px] text-neo-text-muted">{r.summary}</p>
                {r.sample && (
                  <code className="text-[10px] font-mono text-neo-text-muted block bg-neo-surface-high neo-border p-1.5 overflow-x-auto">{JSON.stringify(r.sample)}</code>
                )}
                {tried[key] && <code className="text-[10px] font-mono text-neo-blue block">↳ {tried[key]}</code>}
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}
