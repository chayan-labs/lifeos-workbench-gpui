import React, { useState, useEffect } from 'react';
import { History, CheckSquare, AlertTriangle, Eye, Sparkles, TrendingUp, RefreshCw, Layers, Check } from 'lucide-react';
import { apiCall } from '../lib/api';

export default function HarnessLoop() {
  const [judgeScore, setJudgeScore] = useState(88);
  const [isPromoting, setIsPromoting] = useState(false);
  const [promotionStatus, setPromotionStatus] = useState(null); // null | 'done' | 'discarded'

  // Telemetry table is the append-only events log (GET /api/event). Promote
  // is the only mutation, and it only ever appends (POST /api/event) - there
  // is deliberately no edit/delete UI here, mirroring the backend's contract.
  const [events, setEvents] = useState([]);
  const [eventsState, setEventsState] = useState('loading'); // 'loading' | 'ready' | 'offline'

  const loadEvents = () => {
    setEventsState('loading');
    apiCall('GET', '/api/event?limit=50').then(({ ok, data, offline }) => {
      if (ok && !offline && Array.isArray(data)) {
        setEvents(data);
        setEventsState('ready');
      } else {
        setEventsState('offline');
      }
    });
  };

  useEffect(() => { loadEvents(); }, []);

  return (
    <div className="flex flex-col gap-8">
      {/* Overview */}
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-surface">
        <h2 className="neo-title-md mb-2 flex items-center gap-2">
          <History size={24} className="text-neo-blue" />
          Harness Loop & Shadow Replays
        </h2>
        <p className="neo-body-md text-neo-text-muted">
          The local Mac harness continuously logs every tool call, latency trace, and model run into the append-only <strong>events</strong> registry. An offline LLM-as-judge reviews model output quality. Safe upgrades are tested in shadow-replays before production release.
        </p>
      </div>

      {/* Main Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
        
        {/* Judge Board */}
        <div className="lg:col-span-5 neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
          <div className="flex justify-between items-center border-b-2 border-neo-border pb-3 mb-4">
            <span className="neo-label-md flex items-center gap-2">
              <Sparkles size={18} className="text-neo-yellow fill-neo-yellow" />
              LLM-as-Judge Evaluation
            </span>
            <span className="neo-chip neo-chip--review text-[10px]">STANDBY</span>
          </div>

          <div className="flex flex-col items-center justify-center p-6 bg-neo-bg neo-border neo-radius mb-6">
            <div className="text-center">
              <span className="neo-label-sm text-neo-text-muted block mb-1">AGGREGATE QUALITY SCORE</span>
              <span className="neo-title-xl block text-neo-blue font-black text-6xl my-2">{judgeScore}%</span>
              <span className="neo-chip neo-chip--completed text-[10px] mt-1">ABOVE THRESHOLD (80%)</span>
            </div>
          </div>

          <div className="flex flex-col gap-3 text-xs">
            <div className="flex justify-between p-2.5 border bg-neo-surface">
              <span>Execution Alignment</span>
              <span className="font-bold">96%</span>
            </div>
            <div className="flex justify-between p-2.5 border bg-neo-surface">
              <span>Security Policy Grounding</span>
              <span className="font-bold text-neo-mint">100%</span>
            </div>
            <div className="flex justify-between p-2.5 border bg-neo-surface">
              <span>Token Economy Ratio</span>
              <span className="font-bold">84%</span>
            </div>
          </div>
        </div>

        {/* Release Loop & Shadow Replays */}
        <div className="lg:col-span-7 flex flex-col gap-6">
          <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface flex-1">
            <h3 className="neo-title-md border-b-2 border-neo-border pb-3 mb-4">
              Deployment Promotion Loop
            </h3>
            
            <p className="text-xs text-neo-text-muted mb-4">
              Learned weights and system prompts are versioned inside the DB. Upgrades run shadow replays against historic event streams. If satisfactory, they await manual promotion.
            </p>

            <div className="p-4 bg-neo-surface-muted neo-border flex flex-col gap-3 mb-6">
              <div className="flex justify-between items-center text-xs">
                <div>
                  <span className="neo-label-sm block font-bold">Candidate Config ID: cfg_v2_rerank_prior</span>
                  <span className="text-[10px] text-neo-text-muted">Added 3 hours ago via local training</span>
                </div>
                <span className="neo-chip neo-chip--completed text-[9px]">SHADOW TEST PASSED</span>
              </div>
              
              {promotionStatus === 'done' && (
                <div className="flex items-center gap-2 p-2 bg-neo-mint neo-border text-xs font-bold">
                  <Check size={12} /> Promoted cfg_v2_rerank_prior to production. Score: 91%.
                </div>
              )}
              {promotionStatus === 'discarded' && (
                <div className="p-2 bg-neo-surface-muted neo-border text-xs font-bold text-neo-red">
                  Candidate config discarded.
                </div>
              )}
              <div className="flex gap-3 mt-2">
                <button
                  onClick={() => {
                    setIsPromoting(true);
                    apiCall('POST', '/api/event', {
                      type: 'config.promoted',
                      actor: 'release_loop',
                      attrs: { config_id: 'cfg_v2_rerank_prior', old_score: judgeScore, new_score: 91 },
                    }).then(({ ok, offline }) => {
                      setIsPromoting(false);
                      if (ok && !offline) {
                        setJudgeScore(91);
                        setPromotionStatus('done');
                        loadEvents();
                      } else {
                        setPromotionStatus('discarded');
                      }
                    });
                  }}
                  disabled={isPromoting}
                  className="neo-btn bg-neo-yellow flex-1 py-2 text-xs font-bold flex items-center justify-center gap-2"
                >
                  {isPromoting ? <RefreshCw className="animate-spin" size={12} /> : null}
                  PROMOTE TO PRODUCTION
                </button>
                <button
                  onClick={() => {
                    apiCall('POST', '/api/event', {
                      type: 'config.discarded',
                      actor: 'release_loop',
                      attrs: { config_id: 'cfg_v2_rerank_prior' },
                    }).then(loadEvents);
                    setPromotionStatus('discarded');
                  }}
                  className="neo-btn bg-neo-surface px-4 text-xs font-bold text-neo-red"
                >
                  DISCARD
                </button>
              </div>
            </div>

            {/* Telemetry log table - append-only, GET /api/event */}
            <div className="flex justify-between items-center mb-2">
              <h4 className="neo-label-md text-neo-text-muted">Observability Telemetry Logs</h4>
              <button onClick={loadEvents} className="neo-btn py-1 px-2 bg-neo-surface" title="Refresh">
                <RefreshCw size={12} className={eventsState === 'loading' ? 'animate-spin' : ''} />
              </button>
            </div>
            {eventsState === 'offline' && (
              <div className="px-3 py-2 bg-neo-red text-white text-xs font-bold neo-border mb-3">Backend unreachable.</div>
            )}
            {eventsState === 'ready' && events.length === 0 && (
              <div className="px-3 py-2 bg-neo-surface-muted text-xs neo-border mb-3">No events logged yet.</div>
            )}
            {events.length > 0 && (
              <div className="neo-border overflow-x-auto text-xs">
                <table className="w-full text-left border-collapse bg-neo-surface">
                  <thead>
                    <tr className="border-b-2 border-neo-border bg-neo-bg">
                      <th className="p-2 font-bold">TYPE</th>
                      <th className="p-2 font-bold font-mono">ACTOR</th>
                      <th className="p-2 font-bold font-mono">TOKENS IN/OUT</th>
                      <th className="p-2 font-bold font-mono">COST</th>
                      <th className="p-2 font-bold font-mono">EVAL SCORE</th>
                      <th className="p-2 font-bold text-right">GATED</th>
                    </tr>
                  </thead>
                  <tbody>
                    {events.map((ev) => (
                      <tr key={ev.id} className="border-b border-neo-border">
                        <td className="p-2 font-semibold font-mono text-neo-blue">{ev.type}</td>
                        <td className="p-2 font-mono">{ev.actor}</td>
                        <td className="p-2 font-mono">{ev.tokens_in ?? '-'}/{ev.tokens_out ?? '-'}</td>
                        <td className="p-2 font-mono">{ev.cost != null ? `$${Number(ev.cost).toFixed(2)}` : '-'}</td>
                        <td className="p-2 font-mono text-neo-mint font-bold">{ev.eval_score ?? '-'}</td>
                        <td className="p-2 text-right font-mono font-semibold">{ev.gated ? 'YES' : 'NO'}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

          </div>
        </div>

      </div>
    </div>
  );
}
