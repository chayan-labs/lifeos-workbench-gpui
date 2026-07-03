import React from 'react';
import {
  ResponsiveContainer, LineChart, Line, BarChart, Bar, FunnelChart, Funnel,
  XAxis, YAxis, Tooltip, CartesianGrid, LabelList,
} from 'recharts';
import { computeMetric } from './computeMetric';

const COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#14b8a6'];

// Generic metric renderer: a manifest `metrics` entry + a window of raw
// events in, the declared `viz` (line/bar/funnel) out - no per-module chart
// code. See docs/PLATFORM-SYSTEMS.md and core/metrics/computeMetric.js.
export default function GenericMetricChart({ metric, events, height = 220 }) {
  let data = computeMetric(events, metric);

  // `metric.cumulative: true` turns a per-bucket sum into a running total -
  // e.g. an equity curve from per-day P&L (docs/MODULES.md §2.4 Trading).
  // Still zero bespoke code per module: any manifest's bucketed metric can
  // opt in with this one flag.
  if (metric.cumulative) {
    let running = 0;
    data = data.map((d) => { running += d.value; return { ...d, value: running }; });
  }

  if (!data.length) {
    return <p className="text-xs text-neo-text-muted">No data yet for "{metric.id}".</p>;
  }

  if (metric.viz === 'funnel') {
    return (
      <ResponsiveContainer width="100%" height={height}>
        <FunnelChart>
          <Tooltip />
          <Funnel dataKey="value" data={data} isAnimationActive={false}>
            <LabelList position="right" dataKey="label" fill="#1c1c0f" stroke="none" fontSize={11} />
            {data.map((_, i) => <React.Fragment key={i} />)}
          </Funnel>
        </FunnelChart>
      </ResponsiveContainer>
    );
  }

  if (metric.viz === 'bar') {
    return (
      <ResponsiveContainer width="100%" height={height}>
        <BarChart data={data}>
          <CartesianGrid strokeDasharray="3 3" />
          <XAxis dataKey="label" tick={{ fontSize: 10 }} />
          <YAxis tick={{ fontSize: 10 }} />
          <Tooltip />
          <Bar dataKey="value" fill={COLORS[0]} />
        </BarChart>
      </ResponsiveContainer>
    );
  }

  // Default: line.
  return (
    <ResponsiveContainer width="100%" height={height}>
      <LineChart data={data}>
        <CartesianGrid strokeDasharray="3 3" />
        <XAxis dataKey="label" tick={{ fontSize: 10 }} />
        <YAxis tick={{ fontSize: 10 }} />
        <Tooltip />
        <Line type="monotone" dataKey="value" stroke={COLORS[0]} strokeWidth={2} dot={false} />
      </LineChart>
    </ResponsiveContainer>
  );
}
