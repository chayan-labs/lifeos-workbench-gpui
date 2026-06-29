import React, { useState, useEffect } from 'react';
import { Database as DbIcon, Share2, Type, ArrowRight, Play, RefreshCw, Plus, CheckCircle, Database } from 'lucide-react';

export default function DatabaseView() {
  const [selectedEntity, setSelectedEntity] = useState('trade');
  const [jobs, setJobs] = useState([
    { id: 'job_ingest_001', kind: 'ingest', payload: '{"video_url":"https://r2.lifeos.db/clips/session_92.mp4"}', status: 'queued', priority: 2 },
    { id: 'job_build_308', kind: 'module_build', payload: '{"module":"health"}', status: 'running', priority: 5 },
    { id: 'job_eval_122', kind: 'eval', payload: '{"run_id":"run_49a_sonnet"}', status: 'done', priority: 1 },
    { id: 'job_oauth_019', kind: 'pipeline', payload: '{"sync":"figma_tokens"}', status: 'failed', priority: 3 }
  ]);

  // Load custom entities from localStorage to support persistency in MVP
  const [customEntities, setCustomEntities] = useState([]);
  const [formModule, setFormModule] = useState('trading');
  const [formType, setFormType] = useState('trade');
  const [formTitle, setFormTitle] = useState('AAPL setup');
  const [formAttrs, setFormAttrs] = useState('{\n  "ticker": "AAPL",\n  "pnl": 120.00,\n  "notes": "Broke out of bull flag"\n}');
  const [apiStatus, setApiStatus] = useState('checking');

  useEffect(() => {
    const saved = localStorage.getItem('life_os_custom_entities');
    if (saved) {
      try {
        setCustomEntities(JSON.parse(saved));
      } catch (e) {
        console.error(e);
      }
    }

    // Ping lifeos-api Axum server to check if it's awake
    fetch('http://127.0.0.1:8080/api/health')
      .then(res => res.json())
      .then(data => {
        if (data.status === 'healthy') {
          setApiStatus('online');
        } else {
          setApiStatus('offline');
        }
      })
      .catch(() => setApiStatus('offline'));
  }, []);

  const triggerJobRun = (jobId) => {
    setJobs((prevJobs) => 
      prevJobs.map((job) => 
        job.id === jobId ? { ...job, status: 'running' } : job
      )
    );
    setTimeout(() => {
      setJobs((prevJobs) => 
        prevJobs.map((job) => 
          job.id === jobId ? { ...job, status: 'done' } : job
        )
      );
    }, 1500);
  };

  const entitiesSchema = [
    { name: 'id', type: 'TEXT (UUID)', desc: 'Primary Key' },
    { name: 'workspace_id', type: 'TEXT', desc: 'Tenant ID for SaaS scaling' },
    { name: 'module', type: 'TEXT', desc: 'e.g. learning, tasks, trading, social, design' },
    { name: 'type', type: 'TEXT', desc: 'e.g. topic, task, project, trade, post, campaign' },
    { name: 'parent_id', type: 'TEXT (Nullable)', desc: 'Self-referencing parent ID for hierarchies' },
    { name: 'status', type: 'TEXT', desc: 'Lifecycle state per module manifest' },
    { name: 'ts', type: 'TIMESTAMP', desc: 'Creation/modification time' },
    { name: 'attrs', type: 'JSON', desc: 'Flexible key-value storage for domain-specific fields' },
  ];

  const entityDefinitions = {
    trade: {
      title: 'Trading Entity',
      module: 'trading',
      type: 'trade',
      status: 'completed',
      attrs: {
        ticker: 'AAPL',
        entry: 172.50,
        exit: 181.20,
        stop_loss: 169.00,
        pnl: 8.70,
        r_multiple: 2.48,
        thesis: 'Double bottom bounce on daily chart backed by volume spike.'
      },
      edges: [
        { label: 'depends_on', target: 'Topic: Technical Analysis' },
        { label: 'uses_asset', target: 'Asset: AAPL screenshot' }
      ]
    },
    task: {
      title: 'Task Entity',
      module: 'tasks',
      type: 'task',
      status: 'review',
      attrs: {
        title: 'Migrate knowledge atlas data',
        due: '2026-06-30',
        priority: 'high',
        assigned_to: 'chayan-aggarwal',
        notes: 'Need to write the atlasAdd wrapper in the shim configuration.'
      },
      edges: [
        { label: 'depends_on', target: 'Project: Life OS Core' }
      ]
    },
    campaign: {
      title: 'Marketing Campaign',
      module: 'marketing',
      type: 'campaign',
      status: 'active',
      attrs: {
        campaign_name: 'Summer Launch 2026',
        channels: ['twitter', 'reddit'],
        budget: 500,
        leads_generated: 48,
        conversion_rate: 0.12
      },
      edges: [
        { label: 'publishes_to', target: 'Social Account: X (@life_os_app)' },
        { label: 'uses_asset', target: 'Asset: Summer Banner SVG' }
      ]
    }
  };

  const handleCreateEntity = (e) => {
    e.preventDefault();
    let parsedAttrs = {};
    try {
      parsedAttrs = JSON.parse(formAttrs);
    } catch (err) {
      alert("Invalid attributes JSON format.");
      return;
    }

    const newId = "ent_" + Math.random().toString(36).substring(2, 10);
    const newEnt = {
      id: newId,
      workspace_id: "default-personal-workspace",
      module: formModule,
      type: formType,
      title: formTitle,
      status: "active",
      attrs: parsedAttrs,
      edges: [],
      created_at: Date.now()
    };

    const updated = [newEnt, ...customEntities];
    setCustomEntities(updated);
    localStorage.setItem('life_os_custom_entities', JSON.stringify(updated));

    // Try posting to local Axum backend
    fetch('http://127.0.0.1:8080/api/entity', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        module: formModule,
        type: formType,
        title: formTitle,
        attrs: parsedAttrs
      })
    })
      .then(res => res.json())
      .then(data => console.log("[Database API] Saved on local server:", data))
      .catch(err => console.warn("[Database API] Local server offline, saved locally only."));

    alert("Entity created successfully!");
  };

  const selectedData = entityDefinitions[selectedEntity] || (customEntities.find(e => e.id === selectedEntity) ? {
    title: customEntities.find(e => e.id === selectedEntity).title,
    module: customEntities.find(e => e.id === selectedEntity).module,
    type: customEntities.find(e => e.id === selectedEntity).type,
    status: customEntities.find(e => e.id === selectedEntity).status,
    attrs: customEntities.find(e => e.id === selectedEntity).attrs,
    edges: []
  } : null);

  return (
    <div className="flex flex-col gap-8">
      {/* Introduction Banner */}
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-white flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
        <div>
          <h2 className="neo-title-md mb-2 flex items-center gap-2">
            <DbIcon size={24} className="text-[var(--neo-blue)]" />
            The Notion Killer: One Generic Table
          </h2>
          <p className="neo-body-md text-[var(--neo-text-muted)]">
            Life OS stores all domain models in a single, indexable, multi-tenant <strong>entities</strong> table. New domains require <strong>zero database migrations</strong>.
          </p>
        </div>
        <div className={`neo-tag text-xs font-mono font-bold flex items-center gap-1.5 ${
          apiStatus === 'online' ? 'bg-[var(--neo-mint)]' : 'bg-[var(--neo-red)] text-white'
        }`}>
          <div className={`w-2.5 h-2.5 rounded-full ${apiStatus === 'online' ? 'bg-black animate-pulse' : 'bg-white'}`} />
          <span>Local API: {apiStatus.toUpperCase()}</span>
        </div>
      </div>

      {/* Main interactive grid */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
        
        {/* Core Database Columns */}
        <div className="lg:col-span-4 neo-surface neo-border-thick neo-shadow p-5 bg-white">
          <h3 className="neo-title-md border-b-2 border-[var(--neo-border)] pb-3 mb-4 flex items-center gap-2">
            <Type size={18} />
            `entities` Schema
          </h3>
          <div className="flex flex-col gap-3">
            {entitiesSchema.map((col, idx) => (
              <div key={idx} className="p-3 bg-[var(--neo-surface-muted)] neo-border flex justify-between items-center gap-4">
                <div>
                  <span className="neo-label-md font-mono text-[var(--neo-blue)]">{col.name}</span>
                  <p className="text-xs text-[var(--neo-text-muted)] mt-1">{col.desc}</p>
                </div>
                <span className="neo-label-sm bg-white neo-border px-1.5 py-0.5 text-[10px]">{col.type}</span>
              </div>
            ))}
          </div>
        </div>

        {/* Dynamic Mapping Playground & Custom Entities Form */}
        <div className="lg:col-span-8 flex flex-col gap-6">
          <div className="neo-surface neo-border-thick neo-shadow p-5 bg-white">
            <h3 className="neo-title-md border-b-2 border-[var(--neo-border)] pb-3 mb-4">
              Row Translator & Live Viewer
            </h3>
            
            {/* Entity Selectors */}
            <div className="flex flex-wrap gap-2 mb-6">
              {['trade', 'task', 'campaign'].map((type) => (
                <button
                  key={type}
                  onClick={() => setSelectedEntity(type)}
                  className={`neo-btn py-1.5 px-3 neo-label-sm ${
                    selectedEntity === type ? 'bg-[var(--neo-yellow)]' : 'bg-white'
                  }`}
                >
                  {type.toUpperCase()} (SEED)
                </button>
              ))}

              {customEntities.map((ent) => (
                <button
                  key={ent.id}
                  onClick={() => setSelectedEntity(ent.id)}
                  className={`neo-btn py-1.5 px-3 neo-label-sm ${
                    selectedEntity === ent.id ? 'bg-[var(--neo-mint)]' : 'bg-white'
                  }`}
                >
                  {ent.title.toUpperCase()} (CUSTOM)
                </button>
              ))}
            </div>

            {/* Simulated Database Row */}
            {selectedData && (
              <div className="neo-border p-4 bg-gray-950 text-emerald-400 font-mono text-sm neo-radius overflow-x-auto shadow-inner">
                <div className="text-xs text-gray-500 mb-2">// Simulated SQL row in entities table</div>
                <div><span className="text-pink-400">id</span>: "{selectedEntity.startsWith('ent_') ? selectedEntity : 'd3b07384-d113-4cd4'}"</div>
                <div><span className="text-pink-400">workspace_id</span>: "personal_workspace"</div>
                <div><span className="text-pink-400">module</span>: "{selectedData.module}"</div>
                <div><span className="text-pink-400">type</span>: "{selectedData.type}"</div>
                <div><span className="text-pink-400">status</span>: "{selectedData.status}"</div>
                <div><span className="text-pink-400">attrs</span>: {'{'}</div>
                {Object.entries(selectedData.attrs).map(([key, val]) => (
                  <div key={key} className="pl-4">
                    <span className="text-sky-400">"{key}"</span>: {typeof val === 'number' ? <span className="text-yellow-400">{val}</span> : <span className="text-orange-300">"{val}"</span>},
                  </div>
                ))}
                <div>{'}'}</div>
              </div>
            )}

            {/* Linked Edges (Graph Layer) */}
            {selectedData && selectedData.edges && selectedData.edges.length > 0 && (
              <div className="mt-6">
                <h4 className="neo-label-md mb-3 text-[var(--neo-text-muted)]">Cross-Domain Graph Edges (`edges` table)</h4>
                <div className="flex flex-col gap-2">
                  {selectedData.edges.map((edge, idx) => (
                    <div key={idx} className="flex items-center gap-3 p-2 bg-[var(--neo-surface-muted)] neo-border text-xs font-semibold">
                      <span className="neo-chip neo-chip--active py-0.5 text-[9px]">{selectedData.title}</span>
                      <div className="flex items-center gap-1 text-[var(--neo-red)]">
                        <Share2 size={12} />
                        <span className="font-mono">{edge.label}</span>
                      </div>
                      <ArrowRight size={14} />
                      <span className="neo-chip neo-chip--draft py-0.5 text-[9px]">{edge.target}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>

          {/* Form to Add Entity */}
          <div className="neo-surface neo-border-thick neo-shadow p-5 bg-white">
            <h3 className="neo-title-md border-b-2 border-black pb-3 mb-4 flex items-center gap-2">
              <Plus size={18} />
              Insert Live Row Entity
            </h3>

            <form onSubmit={handleCreateEntity} className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="flex flex-col gap-2">
                <label className="neo-label-sm">Module Domain</label>
                <select 
                  value={formModule} 
                  onChange={(e) => setFormModule(e.target.value)}
                  className="p-2 neo-border bg-white text-xs font-semibold focus:outline-none"
                >
                  <option value="trading">trading</option>
                  <option value="tasks">tasks</option>
                  <option value="marketing">marketing</option>
                  <option value="learning">learning</option>
                </select>
              </div>

              <div className="flex flex-col gap-2">
                <label className="neo-label-sm">Entity Type</label>
                <input 
                  type="text" 
                  value={formType} 
                  onChange={(e) => setFormType(e.target.value)}
                  className="p-2 neo-border bg-white text-xs font-mono"
                />
              </div>

              <div className="flex flex-col gap-2 md:col-span-2">
                <label className="neo-label-sm">Display Title</label>
                <input 
                  type="text" 
                  value={formTitle} 
                  onChange={(e) => setFormTitle(e.target.value)}
                  className="p-2 neo-border bg-white text-xs font-semibold"
                />
              </div>

              <div className="flex flex-col gap-2 md:col-span-2">
                <label className="neo-label-sm">Attributes JSON (attrs)</label>
                <textarea 
                  rows={4}
                  value={formAttrs} 
                  onChange={(e) => setFormAttrs(e.target.value)}
                  className="p-2 neo-border bg-white text-xs font-mono"
                />
              </div>

              <div className="md:col-span-2">
                <button 
                  type="submit" 
                  className="neo-btn w-full bg-[var(--neo-mint)] py-2.5 px-4 font-bold uppercase text-xs"
                >
                  Create & Save Entity →
                </button>
              </div>
            </form>
          </div>
        </div>

      </div>

      {/* Jobs Queue Section */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 bg-white">
        <h3 className="neo-title-md border-b-2 border-black pb-3 mb-4">
          Jobs Queue Manager (`jobs` table cloud ↔ Mac)
        </h3>
        <div className="flex flex-col gap-3">
          {jobs.map((job) => (
            <div key={job.id} className="p-4 bg-[var(--neo-bg)] neo-border flex flex-col md:flex-row md:items-center justify-between gap-4">
              <div>
                <div className="flex items-center gap-2 mb-1.5">
                  <span className="neo-label-md font-mono text-xs text-[var(--neo-blue)]">{job.id}</span>
                  <span className="neo-chip py-0.5 text-[9px]">PRIORITY: {job.priority}</span>
                  <span className="neo-tag text-[9px]">kind: {job.kind}</span>
                </div>
                <code className="text-[10px] bg-white p-1 border font-mono block text-gray-600 truncate max-w-lg">
                  {job.payload}
                </code>
              </div>
              <div className="flex items-center gap-3">
                <span className={`text-[10px] px-1.5 py-0.5 neo-border font-bold uppercase ${
                  job.status === 'done' ? 'bg-[var(--neo-mint)]' :
                  job.status === 'running' ? 'bg-[var(--neo-yellow)]' :
                  job.status === 'failed' ? 'bg-[var(--neo-red)] text-white' : 'bg-white'
                }`}>
                  {job.status}
                </span>
                {job.status !== 'done' && (
                  <button 
                    onClick={() => triggerJobRun(job.id)}
                    disabled={job.status === 'running'}
                    className="neo-btn py-1 px-3 bg-white text-xs font-bold flex items-center gap-1.5"
                  >
                    {job.status === 'running' ? <RefreshCw className="animate-spin" size={12} /> : <Play size={12} />}
                    {job.status === 'running' ? 'Running' : 'Trigger'}
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
