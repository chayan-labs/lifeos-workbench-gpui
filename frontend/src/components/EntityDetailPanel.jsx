import React, { useState, useEffect } from 'react';
import { X, Share2, Activity } from 'lucide-react';
import { apiCall } from '../lib/api';

// Slide-over detail view for a single entity: attrs, graph relations (edges
// where the entity is src or dst), and its event history. Backed entirely by
// the live API - GET /api/entity/:id, GET /api/edge, GET /api/event.
export default function EntityDetailPanel({ entityId, onClose }) {
  const [entity, setEntity] = useState(null);
  const [edges, setEdges] = useState([]);
  const [events, setEvents] = useState([]);
  const [state, setState] = useState('loading'); // 'loading' | 'ready' | 'offline' | 'not_found'

  useEffect(() => {
    if (!entityId) return;
    let cancelled = false;
    setState('loading');

    Promise.all([
      apiCall('GET', `/api/entity/${entityId}`),
      apiCall('GET', `/api/edge?src_id=${entityId}`),
      apiCall('GET', `/api/edge?dst_id=${entityId}`),
      apiCall('GET', `/api/event?entity_id=${entityId}`),
    ]).then(([entRes, edgeSrc, edgeDst, eventRes]) => {
      if (cancelled) return;
      if (entRes.offline) { setState('offline'); return; }
      if (!entRes.ok) { setState('not_found'); return; }
      setEntity(entRes.data);
      setEdges([...(edgeSrc.data || []), ...(edgeDst.data || [])]);
      setEvents(eventRes.data || []);
      setState('ready');
    });

    return () => { cancelled = true; };
  }, [entityId]);

  if (!entityId) return null;

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <div className="absolute inset-0 bg-black/40" onClick={onClose} />
      <div className="relative w-full max-w-lg h-full bg-neo-surface neo-border-thick border-r-0 shadow-2xl overflow-y-auto p-6 flex flex-col gap-6">
        <div className="flex justify-between items-start border-b-2 border-neo-border pb-4">
          <div>
            <h3 className="neo-title-md">{entity?.title || entityId}</h3>
            <span className="text-xs font-mono text-neo-text-muted">{entityId}</span>
          </div>
          <button onClick={onClose} className="neo-icon-btn p-1"><X size={20} /></button>
        </div>

        {state === 'loading' && <p className="text-xs text-neo-text-muted">Loading...</p>}
        {state === 'offline' && <p className="text-xs text-neo-red font-bold">Backend unreachable.</p>}
        {state === 'not_found' && <p className="text-xs text-neo-red font-bold">Entity not found.</p>}

        {state === 'ready' && entity && (
          <>
            <div className="flex gap-2 flex-wrap">
              <span className="neo-chip py-0.5 text-[10px]">module: {entity.module}</span>
              <span className="neo-chip py-0.5 text-[10px]">type: {entity.type}</span>
              <span className="neo-chip py-0.5 text-[10px]">status: {entity.status}</span>
            </div>

            <div>
              <h4 className="neo-label-md mb-2 text-neo-text-muted">Attributes</h4>
              <pre className="neo-border p-3 bg-gray-950 text-emerald-400 font-mono text-xs overflow-x-auto">
{JSON.stringify(entity.attrs || {}, null, 2)}
              </pre>
            </div>

            <div>
              <h4 className="neo-label-md mb-2 text-neo-text-muted flex items-center gap-1.5"><Share2 size={14} /> Relations ({edges.length})</h4>
              {edges.length === 0 ? (
                <p className="text-xs text-neo-text-muted">No edges.</p>
              ) : (
                <div className="flex flex-col gap-2">
                  {edges.map((e) => (
                    <div key={e.id} className="p-2 bg-neo-surface-muted neo-border text-xs font-mono">
                      {e.src_id} --[{e.rel}]--&gt; {e.dst_id || e.dst_ref}
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div>
              <h4 className="neo-label-md mb-2 text-neo-text-muted flex items-center gap-1.5"><Activity size={14} /> Events ({events.length})</h4>
              {events.length === 0 ? (
                <p className="text-xs text-neo-text-muted">No events recorded for this entity.</p>
              ) : (
                <div className="flex flex-col gap-2">
                  {events.map((ev) => (
                    <div key={ev.id} className="p-2 bg-neo-surface-muted neo-border text-xs flex justify-between">
                      <span className="font-mono">{ev.type}</span>
                      <span className="text-neo-text-muted">{ev.actor}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
