import React, { useState, useEffect } from 'react';
import { X, Share2, Activity, UploadCloud } from 'lucide-react';
import { apiCall } from '../lib/api';
import GenericDetail from '../core/renderers/GenericDetail';

// Slide-over detail view for a single entity: attrs, graph relations (edges
// where the entity is src or dst), and its event history. Backed entirely by
// the live API - GET /api/entity/:id, GET /api/edge, GET /api/event.
export default function EntityDetailPanel({ entityId, onClose }) {
  const [entity, setEntity] = useState(null);
  const [edges, setEdges] = useState([]);
  const [events, setEvents] = useState([]);
  const [state, setState] = useState('loading'); // 'loading' | 'ready' | 'offline' | 'not_found'
  const [linkTarget, setLinkTarget] = useState('');
  const [linkRel, setLinkRel] = useState('relates_to');
  const [linkError, setLinkError] = useState('');
  const [pushing, setPushing] = useState(false);
  const [pushResult, setPushResult] = useState(null);

  const loadRelations = (id) => {
    Promise.all([
      apiCall('GET', `/api/edge?src_id=${id}`),
      apiCall('GET', `/api/edge?dst_id=${id}`),
    ]).then(([edgeSrc, edgeDst]) => {
      setEdges([...(edgeSrc.data || []), ...(edgeDst.data || [])]);
    });
  };

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

  const handleLink = (e) => {
    e.preventDefault();
    setLinkError('');
    const target = linkTarget.trim();
    if (!target) return;
    // A target starting with "ent_" is treated as an internal entity id
    // (dst_id); anything else (URL, external key) is an external dst_ref.
    const body = { src_id: entityId, rel: linkRel.trim() || 'relates_to' };
    if (target.startsWith('ent_')) body.dst_id = target; else body.dst_ref = target;

    apiCall('POST', '/api/edge', body).then(({ ok, offline, error }) => {
      if (ok && !offline) {
        setLinkTarget('');
        loadRelations(entityId);
      } else {
        setLinkError(offline ? 'Backend unreachable.' : (error || 'Failed to create edge.'));
      }
    });
  };

  // Notion's "edits propagate back" half (issue #59, docs/MODULES.md §3.4):
  // pushing a mirrored `note` only ever drafts a pending Notion update
  // (services/lifeos-api/src/routes/notion.rs::push) - never writes to
  // Notion directly, same approve->execute queue as every other gated write.
  const handlePushToNotion = () => {
    setPushing(true);
    setPushResult(null);
    apiCall('POST', '/api/notion/push', { entity_id: entityId }).then(({ ok, error, status }) => {
      setPushing(false);
      setPushResult(
        ok
          ? 'Drafted - awaiting approval before it reaches Notion.'
          : status === 501
            ? 'Not configured yet - see docs/MANUAL-SETUP.md.'
            : error || 'Push failed.'
      );
    });
  };

  if (!entityId) return null;

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <div className="absolute inset-0 bg-black/40" onClick={onClose} />
      <div className="relative w-full max-w-lg h-full bg-neo-surface neo-border-thick border-r-0 shadow-2xl overflow-y-auto p-6 flex flex-col gap-6">
        <div className="flex justify-between items-start border-b-2 border-neo-border pb-4">
          <span className="text-xs font-mono text-neo-text-muted">{entityId}</span>
          <button onClick={onClose} className="neo-icon-btn p-1"><X size={20} /></button>
        </div>

        {state === 'loading' && <p className="text-xs text-neo-text-muted">Loading...</p>}
        {state === 'offline' && <p className="text-xs text-neo-red font-bold">Backend unreachable.</p>}
        {state === 'not_found' && <p className="text-xs text-neo-red font-bold">Entity not found.</p>}

        {state === 'ready' && entity && (
          <>
            <GenericDetail entity={entity} display={{ title: 'title' }} />

            {entity.module === 'notion' && entity.type === 'note' && (
              <div>
                <button
                  onClick={handlePushToNotion}
                  disabled={pushing}
                  className="neo-btn py-1.5 px-3 bg-neo-yellow text-xs font-bold flex items-center gap-1.5 disabled:opacity-50"
                >
                  <UploadCloud size={12} /> {pushing ? 'Pushing…' : 'Push to Notion'}
                </button>
                {pushResult && <p className="text-[10px] text-neo-text-muted font-mono mt-1.5">{pushResult}</p>}
              </div>
            )}

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

              <form onSubmit={handleLink} className="flex gap-2 mt-3">
                <input
                  value={linkTarget}
                  onChange={(e) => setLinkTarget(e.target.value)}
                  placeholder="ent_... or external ref/URL"
                  className="p-1.5 neo-border bg-neo-surface text-xs font-mono flex-1"
                />
                <input
                  value={linkRel}
                  onChange={(e) => setLinkRel(e.target.value)}
                  placeholder="rel"
                  className="p-1.5 neo-border bg-neo-surface text-xs font-mono w-28"
                />
                <button type="submit" className="neo-btn py-1.5 px-3 bg-neo-mint text-[10px] font-bold">Link</button>
              </form>
              {linkError && <p className="text-[10px] text-neo-red font-bold mt-1">{linkError}</p>}
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
