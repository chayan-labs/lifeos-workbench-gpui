import React, { useEffect, useState } from 'react';
import { apiCall } from '../lib/api';
import GenericList from './renderers/GenericList';
import GenericTable from './renderers/GenericTable';
import GenericBoard from './renderers/GenericBoard';
import GenericCalendar from './renderers/GenericCalendar';
import GenericDetail from './renderers/GenericDetail';
import EntityDetailPanel from '../components/EntityDetailPanel';

const KIND_RENDERERS = { list: GenericList, table: GenericTable, board: GenericBoard, calendar: GenericCalendar };

// Renders a day-1 module manifest (lib/moduleManifests.js) with zero bespoke
// view code: a view picker over `manifest.views`, each backed by
// `GET /api/entity?module=<id>&type=<view.type>` and rendered through the
// Generic* component matching `view.kind`, driven by `entityTypes.<type>.display`
// (issue #39 - the manifest-driven module system docs/MODULES.md §1 describes).
export default function ModuleManifestPage({ manifest }) {
  const [activeViewId, setActiveViewId] = useState(manifest.views[0]?.id);
  const [entities, setEntities] = useState([]);
  const [state, setState] = useState('loading');
  const [selectedId, setSelectedId] = useState(null);

  const view = manifest.views.find((v) => v.id === activeViewId) || manifest.views[0];
  const entityType = manifest.entityTypes[view?.type] || {};

  useEffect(() => {
    if (!view) return;
    setState('loading');
    apiCall('GET', `/api/entity?module=${encodeURIComponent(manifest.id)}&type=${encodeURIComponent(view.type)}&limit=500`)
      .then(({ ok, data, offline }) => {
        if (offline) { setState('offline'); return; }
        setEntities(ok ? data || [] : []);
        setState('ready');
      });
  }, [manifest.id, view?.type]);

  const Renderer = KIND_RENDERERS[view?.kind] || GenericList;

  return (
    <div className="flex flex-col gap-6">
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-surface">
        <h2 className="neo-title-md mb-1 flex items-center gap-2">
          <span>{manifest.icon}</span> {manifest.name}
        </h2>
        <p className="neo-body-md text-neo-text-muted">
          {Object.values(manifest.entityTypes).length} entity types, {manifest.views.length} views - rendered entirely
          through the generic renderer system, no bespoke UI code per module.
        </p>
      </div>

      <div className="flex gap-2 flex-wrap">
        {manifest.views.map((v) => (
          <button
            key={v.id}
            onClick={() => setActiveViewId(v.id)}
            className={`neo-btn py-1.5 px-3 text-xs font-bold ${view?.id === v.id ? 'bg-neo-yellow' : 'bg-neo-surface'}`}
          >
            {v.label}
          </button>
        ))}
      </div>

      <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
        {state === 'offline' && <p className="text-xs text-neo-red font-bold">Backend unreachable.</p>}
        {state === 'loading' && <p className="text-xs text-neo-text-muted">Loading…</p>}
        {state === 'ready' && view?.kind === 'table' && (
          <Renderer entities={entities} setEntities={setEntities} columns={view.columns} onRowClick={(e) => setSelectedId(e.id)} />
        )}
        {state === 'ready' && view?.kind !== 'table' && (
          <Renderer
            entities={entities}
            setEntities={setEntities}
            display={entityType.display}
            dateField={view?.dateField}
            groupByField={view?.groupBy}
            columns={view?.columns}
            onSelect={(e) => setSelectedId(e.id)}
          />
        )}
      </div>

      {selectedId && <EntityDetailPanel entityId={selectedId} onClose={() => setSelectedId(null)} />}
    </div>
  );
}
