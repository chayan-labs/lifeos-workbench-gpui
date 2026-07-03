import React, { useEffect, useState } from 'react';
import { BarChart3, RefreshCw } from 'lucide-react';
import { apiCall } from '../lib/api';
import GenericMetricChart from '../core/metrics/GenericMetricChart';

// Per-module dashboards generated purely from each module's manifest
// `metrics` declarations (docs/PLATFORM-SYSTEMS.md, docs/MODULES.md §1) - no
// bespoke chart code per module, no new storage (everything aggregates over
// the existing `events` log). Real manifests don't exist yet (that's the
// module-loader epic, #39+), so this ships the same metrics config shape a
// loaded manifest will provide, seeded here for the modules that already
// have live data flowing through `events` this session.
const MODULE_METRICS = {
  tasks: {
    label: 'Tasks / Productivity',
    metrics: [
      { id: 'task_events_by_type', groupBy: 'type', viz: 'bar' },
      { id: 'entity_updates_over_time', where: { type: 'entity.updated' }, bucket: 'day', agg: 'count', viz: 'line' },
    ],
  },
  social: {
    label: 'Social',
    metrics: [
      {
        id: 'post_funnel', viz: 'funnel',
        stages: [
          { label: 'Drafted', where: { type: 'post.drafted' } },
          { label: 'Published', where: { type: 'post.published' } },
        ],
      },
    ],
  },
  harness: {
    label: 'Harness Loop',
    metrics: [
      { id: 'gated_vs_total', groupBy: 'gated', viz: 'bar' },
    ],
  },
};

export default function ModuleDashboards() {
  const [moduleId, setModuleId] = useState('tasks');
  const [events, setEvents] = useState([]);
  const [state, setState] = useState('loading');

  const load = () => {
    setState('loading');
    apiCall('GET', '/api/event?limit=2000').then(({ ok, data, offline }) => {
      if (offline) { setState('offline'); return; }
      setEvents(ok ? data || [] : []);
      setState('ready');
    });
  };

  useEffect(() => { load(); }, []);

  const config = MODULE_METRICS[moduleId];

  return (
    <div className="flex flex-col gap-6">
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-surface">
        <h2 className="neo-title-md mb-2 flex items-center gap-2">
          <BarChart3 size={22} /> Module Dashboards
        </h2>
        <p className="neo-body-md text-neo-text-muted">
          Every chart below is computed client-side from a metric config ({'{ id, where, agg, bucket?, groupBy?, viz }'})
          over the live <code>events</code> log - declaring a new metric in a module's manifest is the only
          work needed to get a working dashboard.
        </p>
      </div>

      <div className="flex items-center gap-3">
        <select
          value={moduleId}
          onChange={(e) => setModuleId(e.target.value)}
          className="p-2 neo-border bg-neo-surface text-xs font-bold"
        >
          {Object.entries(MODULE_METRICS).map(([id, m]) => (
            <option key={id} value={id}>{m.label}</option>
          ))}
        </select>
        <button onClick={load} className="neo-btn bg-neo-surface-high py-2 px-3 flex items-center gap-1.5 text-xs font-bold">
          <RefreshCw size={14} /> Refresh
        </button>
        {state === 'offline' && <span className="text-xs text-neo-red font-bold">Backend unreachable.</span>}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {state === 'ready' && config.metrics.map((metric) => (
          <div key={metric.id} className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
            <h3 className="neo-label-md mb-3">{metric.id}</h3>
            <GenericMetricChart metric={metric} events={events} />
          </div>
        ))}
      </div>
    </div>
  );
}
