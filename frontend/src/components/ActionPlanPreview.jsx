import React, { useEffect, useState } from 'react';
import { Check, X, Lock, Play } from 'lucide-react';
import { apiCall } from '../lib/api';
import { executeAction } from '../lib/agentActions';

// Renders a compiled ActionPlan (actionPlanCompiler.js) as a per-step
// before/after diff + classification, with Apply (allowed) or Approve
// (gated) controls per docs/AGENT-CONTROL.md §3/§6. A full VCS-diff widget
// reuse is deferred until lifeos-vcs ships (later epic) - this renders a
// plain JSON before/after diff in the meantime, which is the same
// information a semantic-diff widget would otherwise wrap.
export default function ActionPlanPreview({ plan, onDone, planId }) {
  const [befores, setBefores] = useState({});
  const [results, setResults] = useState({});

  useEffect(() => {
    // Pre-fetch "before" state for entity.update steps so the diff is real,
    // not just the proposed patch in isolation.
    plan.forEach((step, i) => {
      if (step.tool === 'entity.update' && step.args?.id) {
        apiCall('GET', `/api/entity/${step.args.id}`).then(({ ok, data }) => {
          if (ok) setBefores((prev) => ({ ...prev, [i]: data }));
        });
      }
    });
  }, [plan]);

  const runStep = async (step, i, approved) => {
    const result = await executeAction({ tool: step.tool, args: step.args }, { approved, planId });
    setResults((prev) => ({ ...prev, [i]: result }));
  };

  if (!plan.length) {
    return <p className="text-xs text-neo-text-muted">Nothing actionable was compiled from that instruction.</p>;
  }

  return (
    <div className="flex flex-col gap-3">
      {plan.map((step, i) => {
        const result = results[i];
        const before = befores[i];
        return (
          <div key={i} className="neo-border bg-neo-bg p-3 flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <span className="font-mono text-xs font-bold">{step.tool}</span>
              <span
                className={`neo-tag text-[9px] ${
                  step.classification === 'gated' ? 'bg-neo-yellow text-neo-text'
                  : step.classification === 'forbidden' ? 'bg-neo-red text-white'
                  : 'bg-neo-mint text-neo-text'
                }`}
              >
                {step.classification}
              </span>
            </div>
            {step.reason && <p className="text-[10px] text-neo-text-muted italic">{step.reason}</p>}

            {step.tool === 'entity.update' && (
              <div className="grid grid-cols-2 gap-2 text-[10px] font-mono">
                <div>
                  <div className="text-neo-text-muted mb-1">before</div>
                  <pre className="neo-border bg-gray-950 text-rose-400 p-2 overflow-x-auto">{JSON.stringify(before || {}, null, 2)}</pre>
                </div>
                <div>
                  <div className="text-neo-text-muted mb-1">after (patch)</div>
                  <pre className="neo-border bg-gray-950 text-emerald-400 p-2 overflow-x-auto">{JSON.stringify(step.args?.patch || {}, null, 2)}</pre>
                </div>
              </div>
            )}
            {step.tool !== 'entity.update' && (
              <pre className="neo-border bg-gray-950 text-emerald-400 p-2 text-[10px] font-mono overflow-x-auto">{JSON.stringify(step.args, null, 2)}</pre>
            )}

            {!result && step.classification === 'allowed' && (
              <button onClick={() => runStep(step, i, false)} className="neo-btn bg-neo-mint py-1.5 text-[10px] font-bold self-start flex items-center gap-1">
                <Play size={11} /> Apply
              </button>
            )}
            {!result && step.classification === 'gated' && (
              <button onClick={() => runStep(step, i, true)} className="neo-btn bg-neo-yellow py-1.5 text-[10px] font-bold self-start flex items-center gap-1">
                <Lock size={11} /> Approve & Apply
              </button>
            )}
            {result?.status === 'applied' && (
              <span className="text-[10px] font-bold text-emerald-600 flex items-center gap-1"><Check size={11} /> Applied</span>
            )}
            {result?.status === 'failed' && (
              <span className="text-[10px] font-bold text-neo-red flex items-center gap-1"><X size={11} /> Failed: {result.error}</span>
            )}
          </div>
        );
      })}
      {onDone && (
        <button onClick={onDone} className="neo-btn bg-neo-surface-high py-1.5 px-3 text-[10px] font-bold self-end">Close plan</button>
      )}
    </div>
  );
}
