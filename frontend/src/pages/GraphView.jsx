import React, { useEffect, useMemo, useRef, useState } from 'react';
import cytoscape from 'cytoscape';
import coseBilkent from 'cytoscape-cose-bilkent';
import { GitBranch, RefreshCw } from 'lucide-react';
import { apiCall } from '../lib/api';
import EntityDetailPanel from '../components/EntityDetailPanel';

cytoscape.use(coseBilkent);

const EDGE_LIMIT = 1000; // matches the API's own max LIMIT clamp

// Cross-module entity graph: every edge + the entities it touches, rendered
// with Cytoscape. Module/rel filters narrow the query server-side so the
// canvas stays usable as the graph grows into the hundreds of nodes the
// acceptance criteria call for, rather than always pulling everything.
export default function GraphView() {
  const containerRef = useRef(null);
  const cyRef = useRef(null);
  const [edges, setEdges] = useState([]);
  const [entities, setEntities] = useState({}); // id -> entity
  const [state, setState] = useState('loading'); // loading | ready | offline
  const [moduleFilter, setModuleFilter] = useState('');
  const [relFilter, setRelFilter] = useState('');
  const [selectedId, setSelectedId] = useState(null);

  const load = async () => {
    setState('loading');
    const qp = new URLSearchParams({ limit: String(EDGE_LIMIT) });
    if (relFilter) qp.set('rel', relFilter);
    const edgeRes = await apiCall('GET', `/api/edge?${qp.toString()}`);
    if (edgeRes.offline) { setState('offline'); return; }
    const liveEdges = edgeRes.data || [];

    // Pull every entity referenced by an edge so nodes can show real titles
    // and be filtered by module, not just bare ids.
    const ids = [...new Set(liveEdges.flatMap((e) => [e.src_id, e.dst_id]).filter(Boolean))];
    const fetched = await Promise.all(ids.map((id) => apiCall('GET', `/api/entity/${id}`)));
    const byId = {};
    fetched.forEach((res, i) => { if (res.ok) byId[ids[i]] = res.data; });

    setEdges(liveEdges);
    setEntities(byId);
    setState('ready');
  };

  useEffect(() => { load(); }, [relFilter]);

  const visibleEdges = useMemo(() => {
    if (!moduleFilter) return edges;
    return edges.filter((e) => {
      const src = entities[e.src_id];
      const dst = entities[e.dst_id];
      return src?.module === moduleFilter || dst?.module === moduleFilter;
    });
  }, [edges, entities, moduleFilter]);

  const modules = useMemo(
    () => [...new Set(Object.values(entities).map((e) => e.module))].sort(),
    [entities]
  );

  useEffect(() => {
    if (!containerRef.current || state !== 'ready') return;

    const nodeIds = new Set();
    const elements = [];
    for (const edge of visibleEdges) {
      for (const id of [edge.src_id, edge.dst_id]) {
        if (!id || nodeIds.has(id)) continue;
        nodeIds.add(id);
        const ent = entities[id];
        elements.push({
          data: { id, label: ent?.title || id, module: ent?.module || 'external' },
        });
      }
      elements.push({
        data: {
          id: edge.id,
          source: edge.src_id,
          target: edge.dst_id || edge.dst_ref,
          label: edge.rel,
        },
      });
    }
    // An edge can point at an external dst_ref with no entity row - add a
    // lightweight placeholder node so the edge still renders.
    for (const edge of visibleEdges) {
      const extId = edge.dst_id || edge.dst_ref;
      if (extId && !nodeIds.has(extId)) {
        nodeIds.add(extId);
        elements.unshift({ data: { id: extId, label: extId, module: 'external' } });
      }
    }

    cyRef.current?.destroy();
    const cy = cytoscape({
      container: containerRef.current,
      elements,
      style: [
        { selector: 'node', style: {
          'background-color': '#3b82f6',
          'label': 'data(label)',
          'font-size': 8,
          'color': '#1c1c0f',
          'text-valign': 'bottom',
          'text-margin-y': 4,
          'width': 18,
          'height': 18,
        } },
        { selector: 'edge', style: {
          'width': 1.5,
          'line-color': '#9ca3af',
          'target-arrow-color': '#9ca3af',
          'target-arrow-shape': 'triangle',
          'curve-style': 'bezier',
          'label': 'data(label)',
          'font-size': 6,
          'color': '#6b7280',
        } },
      ],
      layout: { name: elements.length > 300 ? 'cose' : 'cose-bilkent', animate: false, fit: true },
    });
    cy.on('tap', 'node', (evt) => {
      const id = evt.target.id();
      if (id.startsWith('ent_')) setSelectedId(id);
    });
    cyRef.current = cy;

    return () => cy.destroy();
  }, [visibleEdges, entities, state]);

  return (
    <div className="flex flex-col gap-6">
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-surface">
        <h2 className="neo-title-md mb-2 flex items-center gap-2">
          <GitBranch size={22} /> Cross-Module Graph
        </h2>
        <p className="neo-body-md text-neo-text-muted">
          Every relation in <code>edges</code> across every module, in one canvas. Click a node to open its entity detail.
        </p>
      </div>

      <div className="neo-surface neo-border-thick neo-shadow p-4 bg-neo-surface flex flex-wrap items-center gap-3">
        <select
          value={moduleFilter}
          onChange={(e) => setModuleFilter(e.target.value)}
          className="p-2 neo-border bg-neo-surface text-xs font-bold"
        >
          <option value="">All modules</option>
          {modules.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
        <input
          value={relFilter}
          onChange={(e) => setRelFilter(e.target.value)}
          placeholder="Filter by rel (e.g. depends_on)"
          className="p-2 neo-border bg-neo-surface text-xs font-mono flex-1 min-w-[160px]"
        />
        <button onClick={load} className="neo-btn bg-neo-surface-high py-2 px-3 flex items-center gap-1.5 text-xs font-bold">
          <RefreshCw size={14} /> Refresh
        </button>
        <span className="text-xs text-neo-text-muted ml-auto">{visibleEdges.length} edges</span>
      </div>

      <div className="neo-surface neo-border-thick neo-shadow bg-neo-surface p-2">
        {state === 'offline' && (
          <p className="text-xs text-neo-red font-bold p-4">Backend unreachable - the graph needs a live /api/edge.</p>
        )}
        {state === 'ready' && visibleEdges.length === 0 && (
          <p className="text-xs text-neo-text-muted p-4">No edges match these filters yet. Link entities from the Database entity detail panel to populate the graph.</p>
        )}
        <div ref={containerRef} style={{ height: 560, width: '100%' }} />
      </div>

      {selectedId && <EntityDetailPanel entityId={selectedId} onClose={() => setSelectedId(null)} />}
    </div>
  );
}
