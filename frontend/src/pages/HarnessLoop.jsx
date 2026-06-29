import React, { useState } from 'react';
import { History, CheckSquare, AlertTriangle, Eye, Sparkles, TrendingUp, RefreshCw, Layers } from 'lucide-react';

export default function HarnessLoop() {
  const [judgeScore, setJudgeScore] = useState(88);
  const [isPromoting, setIsPromoting] = useState(false);

  const telemetryData = [
    { module: 'trading', tokenCost: '$0.12', latency: '2.1s', judge: '94/100', eventsCount: 14 },
    { module: 'social', tokenCost: '$0.06', latency: '1.4s', judge: '88/100', eventsCount: 8 },
    { module: 'scaffold', tokenCost: '$0.45', latency: '8.7s', judge: '98/100', eventsCount: 4 },
    { module: 'learning', tokenCost: '$0.02', latency: '0.8s', judge: '92/100', eventsCount: 22 },
  ];

  return (
    <div className="flex flex-col gap-8">
      {/* Overview */}
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-white">
        <h2 className="neo-title-md mb-2 flex items-center gap-2">
          <History size={24} className="text-[var(--neo-blue)]" />
          Harness Loop & Shadow Replays
        </h2>
        <p className="neo-body-md text-[var(--neo-text-muted)]">
          The local Mac harness continuously logs every tool call, latency trace, and model run into the append-only <strong>events</strong> registry. An offline LLM-as-judge reviews model output quality. Safe upgrades are tested in shadow-replays before production release.
        </p>
      </div>

      {/* Main Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
        
        {/* Judge Board */}
        <div className="lg:col-span-5 neo-surface neo-border-thick neo-shadow p-5 bg-white">
          <div className="flex justify-between items-center border-b-2 border-[var(--neo-border)] pb-3 mb-4">
            <span className="neo-label-md flex items-center gap-2">
              <Sparkles size={18} className="text-[var(--neo-yellow)] fill-[var(--neo-yellow)]" />
              LLM-as-Judge Evaluation
            </span>
            <span className="neo-chip neo-chip--review text-[10px]">STANDBY</span>
          </div>

          <div className="flex flex-col items-center justify-center p-6 bg-[var(--neo-bg)] neo-border neo-radius mb-6">
            <div className="text-center">
              <span className="neo-label-sm text-[var(--neo-text-muted)] block mb-1">AGGREGATE QUALITY SCORE</span>
              <span className="neo-title-xl block text-[var(--neo-blue)] font-black text-6xl my-2">{judgeScore}%</span>
              <span className="neo-chip neo-chip--completed text-[10px] mt-1">ABOVE THRESHOLD (80%)</span>
            </div>
          </div>

          <div className="flex flex-col gap-3 text-xs">
            <div className="flex justify-between p-2.5 border bg-white">
              <span>Execution Alignment</span>
              <span className="font-bold">96%</span>
            </div>
            <div className="flex justify-between p-2.5 border bg-white">
              <span>Security Policy Grounding</span>
              <span className="font-bold text-[var(--neo-mint)]">100%</span>
            </div>
            <div className="flex justify-between p-2.5 border bg-white">
              <span>Token Economy Ratio</span>
              <span className="font-bold">84%</span>
            </div>
          </div>
        </div>

        {/* Release Loop & Shadow Replays */}
        <div className="lg:col-span-7 flex flex-col gap-6">
          <div className="neo-surface neo-border-thick neo-shadow p-5 bg-white flex-1">
            <h3 className="neo-title-md border-b-2 border-[var(--neo-border)] pb-3 mb-4">
              Deployment Promotion Loop
            </h3>
            
            <p className="text-xs text-[var(--neo-text-muted)] mb-4">
              Learned weights and system prompts are versioned inside the DB. Upgrades run shadow replays against historic event streams. If satisfactory, they await manual promotion.
            </p>

            <div className="p-4 bg-[var(--neo-surface-muted)] neo-border flex flex-col gap-3 mb-6">
              <div className="flex justify-between items-center text-xs">
                <div>
                  <span className="neo-label-sm block font-bold">Candidate Config ID: cfg_v2_rerank_prior</span>
                  <span className="text-[10px] text-[var(--neo-text-muted)]">Added 3 hours ago via local training</span>
                </div>
                <span className="neo-chip neo-chip--completed text-[9px]">SHADOW TEST PASSED</span>
              </div>
              
              <div className="flex gap-3 mt-2">
                <button
                  onClick={() => {
                    setIsPromoting(true);
                    setTimeout(() => {
                      setIsPromoting(false);
                      setJudgeScore(91);
                      
                      // Push to local events log
                      const customEvents = JSON.parse(localStorage.getItem('life_os_custom_events') || '[]');
                      customEvents.unshift({
                        id: "ev_" + Math.random().toString(36).substring(2, 9),
                        ts: Date.now(),
                        type: "config.promoted",
                        actor: "release_loop",
                        attrs: { config_id: "cfg_v2_rerank_prior", old_score: 88, new_score: 91 }
                      });
                      localStorage.setItem('life_os_custom_events', JSON.stringify(customEvents));
                      
                      alert("Successfully promoted cfg_v2_rerank_prior to active production config! Active weights rotated atomically.");
                    }, 1200);
                  }}
                  disabled={isPromoting}
                  className="neo-btn bg-[var(--neo-yellow)] flex-1 py-2 text-xs font-bold flex items-center justify-center gap-2"
                >
                  {isPromoting ? <RefreshCw className="animate-spin" size={12} /> : null}
                  PROMOTE TO PRODUCTION
                </button>
                <button className="neo-btn bg-white px-4 text-xs font-bold text-[var(--neo-red)]">
                  DISCARD
                </button>
              </div>
            </div>

            {/* Telemetry log table */}
            <h4 className="neo-label-md mb-2 text-[var(--neo-text-muted)]">Observability Telemetry Logs</h4>
            <div className="neo-border overflow-x-auto text-xs">
              <table className="w-full text-left border-collapse bg-white">
                <thead>
                  <tr className="border-b-2 border-black bg-[var(--neo-bg)]">
                    <th className="p-2 font-bold">MODULE</th>
                    <th className="p-2 font-bold font-mono">TOKEN COST</th>
                    <th className="p-2 font-bold font-mono">LATENCY</th>
                    <th className="p-2 font-bold font-mono">EVAL SCORE</th>
                    <th className="p-2 font-bold text-right">EVENTS</th>
                  </tr>
                </thead>
                <tbody>
                  {telemetryData.map((data, idx) => (
                    <tr key={idx} className="border-b border-gray-100">
                      <td className="p-2 font-semibold font-mono text-[var(--neo-blue)]">{data.module}</td>
                      <td className="p-2 font-mono">{data.tokenCost}</td>
                      <td className="p-2 font-mono">{data.latency}</td>
                      <td className="p-2 font-mono text-emerald-600 font-bold">{data.judge}</td>
                      <td className="p-2 text-right font-mono font-semibold">{data.eventsCount}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

          </div>
        </div>

      </div>
    </div>
  );
}
