import React, { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { BarChart3, Boxes, Sparkles, Store } from 'lucide-react';
import AIEdit from '../components/ui/AIEdit';
import { apiCall } from '../lib/api';
import { MODULE_MANIFESTS } from '../lib/moduleManifests';
import { hydrateFromStorage } from '../lib/moduleRegistry';

// Module index (docs/MODULES.md). This page is a thin directory over the ONE
// real module system: day-1 manifests (lib/moduleManifests.js) and modules
// hot-installed via self-extension (lib/moduleRegistry.js) all render at
// /m/:id through the same generic renderers. The previous version of this
// page was a second, hand-written mock of the same modules with its own
// localStorage data path - removed so there is exactly one source of truth.

const VIEW_KIND_LABELS = {
  list: 'List',
  table: 'Table',
  board: 'Board',
  calendar: 'Calendar',
  gallery: 'Gallery',
  timeline: 'Timeline',
  map: 'Map',
  funnel: 'Funnel',
};

function ModuleCard({ id, name, icon, views, entityCount, installed }) {
  const kinds = [...new Set((views || []).map((v) => VIEW_KIND_LABELS[v.kind] || v.kind))];
  return (
    <Link
      to={`/m/${id}`}
      className="neo-border neo-shadow bg-neo-surface p-4 flex flex-col gap-3 hover:bg-neo-surface-high transition-colors"
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-xl shrink-0">{icon || '📦'}</span>
          <span className="neo-label-md font-bold truncate">{name || id}</span>
        </div>
        {installed && (
          <span className="neo-tag bg-neo-mint text-[9px] font-mono shrink-0 flex items-center gap-1">
            <Sparkles size={9} /> SELF-BUILT
          </span>
        )}
      </div>
      <div className="flex flex-wrap gap-1.5">
        {kinds.length === 0 && <span className="neo-tag text-[9px]">generic list</span>}
        {kinds.map((k) => (
          <span key={k} className="neo-tag text-[9px]">{k}</span>
        ))}
      </div>
      <div className="mt-auto flex items-center justify-between text-[10px] text-neo-text-muted font-mono">
        <span>{entityCount == null ? '—' : `${entityCount} entities`}</span>
        <span>/m/{id}</span>
      </div>
    </Link>
  );
}

export default function Modules() {
  const [installed, setInstalled] = useState(() => hydrateFromStorage());
  const [counts, setCounts] = useState(null);

  useEffect(() => {
    // Hot-installed modules land via the same registry event Layout's SSE
    // hook dispatches - no second stream subscription needed here.
    const onMounted = () => setInstalled(hydrateFromStorage());
    window.addEventListener('lifeos:module-mounted', onMounted);
    return () => window.removeEventListener('lifeos:module-mounted', onMounted);
  }, []);

  useEffect(() => {
    // One fetch, counted client-side per module - same pattern as
    // ModuleDashboards aggregating over the events log.
    apiCall('GET', '/api/entity?limit=2000').then(({ ok, data, offline }) => {
      if (!ok || offline || !Array.isArray(data)) return;
      const byModule = {};
      data.forEach((e) => { byModule[e.module] = (byModule[e.module] || 0) + 1; });
      setCounts(byModule);
    });
  }, []);

  const dayOne = Object.values(MODULE_MANIFESTS);
  const dayOneIds = new Set(dayOne.map((m) => m.id));
  const hotInstalled = installed.filter((m) => !dayOneIds.has(m.id));

  return (
    <div className="flex flex-col gap-6">
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-surface">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h2 className="neo-title-md mb-2 flex items-center gap-2">
              <Boxes size={22} /> Modules
            </h2>
            <p className="neo-body-md text-neo-text-muted max-w-2xl">
              Every module is a declarative manifest over the same generic <code>entities</code> table -
              no bespoke tables, no bespoke views. Open a module to work in it, or ask the AI to
              scaffold a brand-new one.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <AIEdit prefill="Build a new module for " label="Build a module with AI" />
            <Link to="/dashboards" className="neo-btn bg-neo-surface-high py-2 px-3 text-xs font-bold flex items-center gap-1.5">
              <BarChart3 size={13} /> Dashboards
            </Link>
            <Link to="/marketplace" className="neo-btn bg-neo-surface-high py-2 px-3 text-xs font-bold flex items-center gap-1.5">
              <Store size={13} /> Marketplace
            </Link>
          </div>
        </div>
      </div>

      <div>
        <h3 className="neo-label-md mb-3">Day-1 modules</h3>
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {dayOne.map((m) => (
            <ModuleCard
              key={m.id}
              id={m.id}
              name={m.name}
              icon={m.icon}
              views={m.views}
              entityCount={counts ? counts[m.id] || 0 : null}
            />
          ))}
        </div>
      </div>

      {hotInstalled.length > 0 && (
        <div>
          <h3 className="neo-label-md mb-3">Self-built modules</h3>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
            {hotInstalled.map((m) => (
              <ModuleCard
                key={m.id}
                id={m.id}
                name={m.name}
                icon={m.icon}
                views={m.views}
                entityCount={counts ? counts[m.id] || 0 : null}
                installed
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
