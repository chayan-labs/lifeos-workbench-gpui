import React, { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { Sparkles } from 'lucide-react';
import { apiCall } from '../lib/api';
import { getModule } from '../lib/moduleRegistry';
import { getManifest } from '../lib/moduleManifests';
import GenericList from '../core/renderers/GenericList';
import ModuleManifestPage from '../core/ModuleManifestPage';

// Landing page for a module. Day-1 modules (lib/moduleManifests.js, e.g.
// 'learning') have a real manifest and render through ModuleManifestPage's
// generic view system (issue #39). A module hot-installed via the
// self-extension stream (issue #29) has no full manifest yet, so it falls
// back to an honest flat list of its entities instead of pretending to
// have bespoke views.
export default function InstalledModulePage() {
  const { id } = useParams();
  const staticManifest = getManifest(id);
  const manifest = getModule(id);
  const [entities, setEntities] = useState([]);
  const [state, setState] = useState('loading');

  // Hooks must run unconditionally even when a static manifest takes over
  // rendering below - skip the fetch entirely in that case (ModuleManifestPage
  // does its own per-view fetching).
  useEffect(() => {
    if (staticManifest) return;
    apiCall('GET', `/api/entity?module=${encodeURIComponent(id)}`).then(({ ok, data, offline }) => {
      if (offline) { setState('offline'); return; }
      setEntities(ok ? data || [] : []);
      setState('ready');
    });
  }, [id, staticManifest]);

  if (staticManifest) return <ModuleManifestPage manifest={staticManifest} />;

  return (
    <div className="flex flex-col gap-6">
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-surface">
        <h2 className="neo-title-md mb-2 flex items-center gap-2">
          <Sparkles size={22} /> {manifest?.name || id}
        </h2>
        <p className="neo-body-md text-neo-text-muted">
          Hot-installed via self-extension - this generic list renders its entities directly
          ({'module: ' + id}) until a full manifest (views/board/calendar/...) ships for it.
        </p>
      </div>
      <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
        {state === 'offline' && <p className="text-xs text-neo-red font-bold">Backend unreachable.</p>}
        {state === 'ready' && <GenericList entities={entities} display={{ title: 'title', badge: 'type' }} />}
      </div>
    </div>
  );
}
