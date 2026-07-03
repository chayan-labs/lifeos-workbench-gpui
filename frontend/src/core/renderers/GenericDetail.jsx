import React from 'react';
import { resolveDisplay } from './displayHelpers';

// Generic detail renderer: header (title/subtitle/badge from manifest
// display config) + a raw attrs dump. Shared by EntityDetailPanel (the
// app-wide slide-over) and reusable by any module's own detail view - the
// `detail` view kind from docs/MODULES.md §1 never needs bespoke JSX.
export default function GenericDetail({ entity, display = {} }) {
  const { title, subtitle, badge } = resolveDisplay(entity, display);
  return (
    <div className="flex flex-col gap-4">
      <div>
        <h3 className="neo-title-md">{title}</h3>
        {subtitle && <p className="text-xs text-neo-text-muted mt-1">{subtitle}</p>}
      </div>

      <div className="flex gap-2 flex-wrap">
        <span className="neo-chip py-0.5 text-[10px]">module: {entity.module}</span>
        <span className="neo-chip py-0.5 text-[10px]">type: {entity.type}</span>
        <span className="neo-chip py-0.5 text-[10px]">status: {entity.status}</span>
        {badge && <span className="neo-chip py-0.5 text-[10px]">{badge}</span>}
      </div>

      <div>
        <h4 className="neo-label-md mb-2 text-neo-text-muted">Attributes</h4>
        <pre className="neo-border p-3 bg-gray-950 text-emerald-400 font-mono text-xs overflow-x-auto">
{JSON.stringify(entity.attrs || {}, null, 2)}
        </pre>
      </div>
    </div>
  );
}
