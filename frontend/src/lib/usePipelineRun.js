import { useState, useRef, useEffect } from 'react';
import { apiCall } from './api';

const POLL_MS = 2000;

// Extracted from Dashboard.jsx's original inline pipeline-demo polling
// (issue #92) so PipelineBuilder.jsx (issue #94) can trigger + poll any
// registered pipeline, not just the one hardcoded on the Dashboard.
// `stageNames` is the ordered list of stage names to track (from the
// registry, GET /api/pipeline/registry).
export function usePipelineRun(stageNames) {
  const idleLogs = () => stageNames.map((name) => ({ stage: name, status: 'idle' }));

  const [logs, setLogs] = useState(idleLogs());
  const [running, setRunning] = useState(false);
  const [runState, setRunState] = useState(null); // null | 'completed' | 'failed' | 'awaiting_approval' | 'gated'
  const pollRef = useRef(null);

  useEffect(() => () => clearInterval(pollRef.current), []);

  const trigger = async (pipelineId, input = {}) => {
    clearInterval(pollRef.current);
    setRunning(true);
    setRunState(null);
    setLogs(idleLogs());

    const { ok, data, offline } = await apiCall('POST', '/api/pipeline/run', {
      pipeline: pipelineId,
      input,
    });
    if (!ok || offline || !data?.job_id) {
      setRunning(false);
      setRunState('failed');
      return null;
    }
    const runId = data.job_id;

    const poll = async () => {
      const { ok: eventsOk, data: events, offline: eventsOffline } = await apiCall(
        'GET',
        `/api/event?run_id=${encodeURIComponent(runId)}`
      );
      if (!eventsOk || eventsOffline || !Array.isArray(events)) return;

      const byTs = [...events].sort((a, b) => a.ts - b.ts);
      const nextLogs = idleLogs();
      let terminal = null;
      for (const evt of byTs) {
        const stageIdx = stageNames.indexOf(evt.attrs?.stage);
        if (stageIdx === -1) continue;
        if (evt.type === 'pipeline.stage.completed') {
          nextLogs[stageIdx].status = 'done';
        } else if (evt.type === 'pipeline.stage.failed') {
          nextLogs[stageIdx].status = 'failed';
          terminal = 'failed';
        } else if (evt.type === 'pipeline.stage.gated') {
          nextLogs[stageIdx].status = 'gated';
          terminal = evt.outcome === 'awaiting_approval' ? 'awaiting_approval' : 'gated';
        }
      }
      if (!terminal) {
        const runningIdx = nextLogs.findIndex((l) => l.status === 'idle');
        if (runningIdx !== -1) {
          nextLogs[runningIdx].status = 'running';
        } else {
          terminal = 'completed';
        }
      }
      setLogs(nextLogs);
      if (terminal) {
        setRunState(terminal);
        setRunning(false);
        clearInterval(pollRef.current);
      }
    };

    poll();
    pollRef.current = setInterval(poll, POLL_MS);
    return runId;
  };

  return { logs, running, runState, trigger };
}
