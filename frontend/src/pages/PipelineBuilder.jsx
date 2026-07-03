import React, { useState, useEffect, useCallback } from 'react';
import { Cpu, RefreshCw, ChevronDown, ChevronRight } from 'lucide-react';
import { apiCall } from '../lib/api';
import { usePipelineRun } from '../lib/usePipelineRun';

// One registered pipeline: its DAG stages (from GET /api/pipeline/registry)
// plus a live trigger + per-stage status via usePipelineRun.
function PipelineCard({ pipeline, onRunFinished }) {
  const stageNames = pipeline.stages.map((s) => s.name);
  const { logs, running, runState, trigger } = usePipelineRun(stageNames);

  useEffect(() => {
    if (runState) onRunFinished();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runState]);

  const handleTrigger = () => trigger(pipeline.id, {});

  return (
    <div className="neo-surface neo-border p-4 flex flex-col gap-3">
      <div className="flex justify-between items-center">
        <h4 className="neo-label-md font-bold font-mono">{pipeline.id}</h4>
        <button
          onClick={handleTrigger}
          disabled={running}
          className="neo-btn py-1 px-3 bg-neo-yellow text-xs font-bold"
        >
          {running ? 'RUNNING...' : 'TRIGGER'}
        </button>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        {pipeline.stages.map((stage, idx) => (
          <React.Fragment key={stage.name}>
            <span
              className={`text-[10px] px-2 py-1 border font-bold uppercase font-mono ${
                logs[idx]?.status === 'done' ? 'bg-neo-mint' :
                logs[idx]?.status === 'running' ? 'bg-neo-yellow animate-pulse' :
                logs[idx]?.status === 'gated' ? 'bg-neo-red text-white' :
                logs[idx]?.status === 'failed' ? 'bg-neo-red text-white' : 'bg-neo-surface text-neo-text-muted'
              }`}
            >
              {stage.name}{stage.gated ? ' (gated)' : ''}
            </span>
            {idx < pipeline.stages.length - 1 && <span className="text-neo-text-muted">&rarr;</span>}
          </React.Fragment>
        ))}
      </div>
      {runState && (
        <div className="text-[10px] font-mono text-neo-text-muted uppercase">
          Run state: {runState.replace('_', ' ')}
        </div>
      )}
    </div>
  );
}

// One row of run history (a `pipeline_run` entity, issue #93/#92). Expands
// to fetch that run's stage events via `GET /api/event?run_id=<attrs.run_id>`
// (the entity's own `attrs.run_id`, set by `process_pipeline_job` since #94).
function RunHistoryRow({ run, expanded, onToggle }) {
  const [events, setEvents] = useState(null);

  useEffect(() => {
    if (!expanded || events !== null) return;
    const runId = run.attrs?.run_id;
    if (!runId) return;
    apiCall('GET', `/api/event?run_id=${encodeURIComponent(runId)}`).then(({ ok, data, offline }) => {
      if (ok && !offline && Array.isArray(data)) {
        setEvents([...data].sort((a, b) => a.ts - b.ts));
      }
    });
  }, [expanded, events, run]);

  return (
    <>
      <tr className="cursor-pointer hover:bg-neo-surface-muted" onClick={onToggle}>
        <td className="p-2">{expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}</td>
        <td className="p-2 font-mono text-xs">{run.attrs?.pipeline_id || '-'}</td>
        <td className="p-2 text-xs">
          <span className="neo-chip py-0.5 text-[10px]">{run.attrs?.status || 'unknown'}</span>
        </td>
        <td className="p-2 text-xs">{run.created_at ? new Date(run.created_at * 1000).toLocaleString() : '-'}</td>
      </tr>
      {expanded && (
        <tr>
          <td colSpan={4} className="p-2 bg-neo-surface-muted">
            {events === null ? (
              <span className="text-xs text-neo-text-muted">Loading stages...</span>
            ) : events.length === 0 ? (
              <span className="text-xs text-neo-text-muted">No stage events recorded.</span>
            ) : (
              <div className="flex flex-col gap-1">
                {events.map((evt) => (
                  <div key={evt.id} className="flex justify-between text-[10px] font-mono">
                    <span>{evt.attrs?.stage || evt.type}</span>
                    <span className="text-neo-text-muted">
                      {evt.type}{evt.outcome ? ` (${evt.outcome})` : ''}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </td>
        </tr>
      )}
    </>
  );
}

// Issue #94: inspect registered pipelines' DAG stages, trigger a run, and
// browse run history - the registry is a real GET route
// (`services/lifeos-api/src/routes/pipeline.rs`) over the static Rust
// table `lifeos_pipelines::pipeline_registry()`; there is no manifest-driven
// authoring UI yet (deferred, same gap the registry itself documents).
export default function PipelineBuilder() {
  const [pipelines, setPipelines] = useState(null);
  const [pipelinesState, setPipelinesState] = useState('loading'); // 'loading' | 'ready' | 'offline'
  const [history, setHistory] = useState([]);
  const [historyState, setHistoryState] = useState('loading');
  const [expandedRunId, setExpandedRunId] = useState(null);

  useEffect(() => {
    let cancelled = false;
    apiCall('GET', '/api/pipeline/registry').then(({ ok, data, offline }) => {
      if (cancelled) return;
      if (ok && !offline && Array.isArray(data)) {
        setPipelines(data);
        setPipelinesState('ready');
      } else {
        setPipelinesState('offline');
      }
    });
    return () => { cancelled = true; };
  }, []);

  const loadHistory = useCallback(() => {
    apiCall('GET', '/api/entity?type=pipeline_run&limit=50').then(({ ok, data, offline }) => {
      if (ok && !offline && Array.isArray(data)) {
        setHistory(data);
        setHistoryState('ready');
      } else {
        setHistoryState('offline');
      }
    });
  }, []);

  useEffect(() => { loadHistory(); }, [loadHistory]);

  return (
    <div className="flex flex-col gap-6">
      <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
        <div className="flex justify-between items-center border-b-2 border-neo-border pb-3 mb-4">
          <h3 className="neo-title-md flex items-center gap-2">
            <Cpu size={18} />
            Registered Pipelines
          </h3>
        </div>
        {pipelinesState === 'loading' && <p className="text-xs text-neo-text-muted">Loading...</p>}
        {pipelinesState === 'offline' && <p className="text-xs text-neo-text-muted">Backend unreachable.</p>}
        {pipelinesState === 'ready' && (
          <div className="flex flex-col gap-4">
            {pipelines.map((p) => (
              <PipelineCard key={p.id} pipeline={p} onRunFinished={loadHistory} />
            ))}
          </div>
        )}
      </div>

      <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
        <div className="flex justify-between items-center border-b-2 border-neo-border pb-3 mb-4">
          <h3 className="neo-title-md">Run History</h3>
          <button onClick={loadHistory} className="neo-icon-btn p-1.5 border-0 bg-transparent cursor-pointer">
            <RefreshCw size={14} />
          </button>
        </div>
        {historyState === 'offline' && <p className="text-xs text-neo-text-muted">Backend unreachable.</p>}
        {historyState !== 'offline' && history.length === 0 && (
          <p className="text-xs text-neo-text-muted">No pipeline runs yet.</p>
        )}
        {history.length > 0 && (
          <table className="w-full text-left">
            <thead>
              <tr className="border-b-2 border-neo-border text-[10px] uppercase text-neo-text-muted">
                <th className="p-2 w-6"></th>
                <th className="p-2">Pipeline</th>
                <th className="p-2">Status</th>
                <th className="p-2">Created</th>
              </tr>
            </thead>
            <tbody>
              {history.map((run) => (
                <RunHistoryRow
                  key={run.id}
                  run={run}
                  expanded={expandedRunId === run.id}
                  onToggle={() => setExpandedRunId(expandedRunId === run.id ? null : run.id)}
                />
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
