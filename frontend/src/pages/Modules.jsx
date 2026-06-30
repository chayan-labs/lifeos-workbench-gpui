import React, { useState, useEffect } from 'react';
import AIEdit from '../components/ui/AIEdit';
import { apiCall } from '../lib/api';
import GenericBoard from '../core/renderers/GenericBoard';
import {
  GraduationCap, 
  CheckSquare, 
  FolderKanban, 
  TrendingUp, 
  MessageSquare, 
  Megaphone, 
  Palette, 
  Play, 
  RefreshCw,
  Plus,
  ShieldCheck,
  Send,
  Eye,
  Calendar,
  Grid,
  List,
  GitBranch,
  MapPin,
  Clock,
  Compass,
  FileText,
  RotateCw,
  Heart,
  Sparkles
} from 'lucide-react';

export default function ModulesView() {
  const [activeModule, setActiveModule] = useState('tasks');
  const [viewStyle, setViewStyle] = useState('board'); // board, list, calendar, graph, gallery, timeline, map

  // Task Board state. Source of truth is `entities` (module=tasks, type=task)
  // via the live API; localStorage is only the offline fallback shown until a
  // connection succeeds.
  const [tasks, setTasks] = useState([
    { id: 1, title: 'Map ENCODE GraphQL API schema', status: 'IN_PROGRESS', label: 'GENETICS' },
    { id: 2, title: 'Write SQLite FTS5 index trigger', status: 'REVIEW', label: 'CORE' },
    { id: 3, title: 'Setup Nango instance on fly.io', status: 'COMPLETED', label: 'DEVOPS' },
    { id: 4, title: 'Verify broker-guard closed bounds', status: 'OVERDUE', label: 'TRADING' },
  ]);
  const [tasksSource, setTasksSource] = useState('local'); // 'local' | 'api'

  const entityToTask = (ent) => ({
    id: ent.id,
    title: ent.title,
    status: ent.status,
    label: (ent.attrs && ent.attrs.label) || 'CORE',
  });

  const loadTasksFromApi = () => {
    apiCall('GET', '/api/entity?module=tasks&type=task').then(({ ok, data, offline }) => {
      if (ok && !offline && Array.isArray(data) && data.length > 0) {
        setTasks(data.map(entityToTask));
        setTasksSource('api');
      }
    });
  };

  // Social drafts: real `social/post` entities, source of truth for status
  // is the entity row (+ the append-only events trail), not local state -
  // mirrors the same draft -> Telegram approve/deny -> executed gating model
  // (docs/SECURITY.md §2) the in-app surface has to match.
  const [socialDrafts, setSocialDrafts] = useState([]);
  const [socialDraftsSource, setSocialDraftsSource] = useState('local');

  const entityToDraft = (ent) => ({
    id: ent.id,
    platform: (ent.attrs && ent.attrs.platform) || 'X / Twitter',
    account: (ent.attrs && ent.attrs.account) || '@life_os_dev',
    text: ent.title || (ent.attrs && ent.attrs.text) || '',
    status: ent.status || 'drafted',
  });

  const loadSocialDrafts = () => {
    apiCall('GET', '/api/entity?module=social&type=post').then(({ ok, data, offline }) => {
      if (ok && !offline && Array.isArray(data) && data.length > 0) {
        setSocialDrafts(data.map(entityToDraft));
        setSocialDraftsSource('api');
      } else {
        setSocialDrafts([
          { id: 1, platform: 'X / Twitter', account: '@life_os_dev', text: 'Exciting news! Life OS self-extension validation pipeline is officially 100% locally sandboxed. Headless Playwright assertions prevent build leaks.', status: 'drafted' },
          { id: 2, platform: 'Instagram', account: 'life_os_studio', text: 'Behind the scenes: Spinning up custom connectors using self-hosted Nango OAuth vault.', status: 'drafted' },
        ]);
      }
    });
  };

  // Design assets state
  const [assets, setAssets] = useState([
    { name: 'logo_spinning_globe.gif', size: '1.2 MB', color: 'bg-yellow-100', label: 'MARKETING' },
    { name: 'dashboard_v2_mock.png', size: '480 KB', color: 'bg-indigo-100', label: 'DESIGN' },
    { name: 'audio_dictation_notes.wav', size: '12.4 MB', color: 'bg-emerald-100', label: 'LEARNING' },
    { name: 'campaign_banner.svg', size: '120 KB', color: 'bg-rose-100', label: 'MARKETING' }
  ]);

  // Custom design generator state
  const [promptInput, setPromptInput] = useState('Abstract neo-brutalist circle logo');
  const [isGenerating, setIsRunningGen] = useState(false);

  // Dynamic modules list (checking self-extension status)
  const [installedModules, setInstalledModules] = useState([]);

  useEffect(() => {
    // Synchronize tasks
    const savedTasks = localStorage.getItem('life_os_tasks');
    if (savedTasks) {
      try { setTasks(JSON.parse(savedTasks)); } catch (e) { console.error(e); }
    }

    // Synchronize assets
    const savedAssets = localStorage.getItem('life_os_assets');
    if (savedAssets) {
      try { setAssets(JSON.parse(savedAssets)); } catch (e) { console.error(e); }
    }

    // Check if dynamic modules exist
    const isHealthInstalled = localStorage.getItem('life_os_module_health') === 'true';
    if (isHealthInstalled) {
      setInstalledModules([{ id: 'health', label: 'Health Tracker', icon: Heart, color: 'var(--neo-mint)' }]);
    }

    loadTasksFromApi();
    loadSocialDrafts();
  }, []);

  const saveTasks = (newTasks) => {
    setTasks(newTasks);
    if (tasksSource === 'local') localStorage.setItem('life_os_tasks', JSON.stringify(newTasks));
  };

  // Quick form for adding task
  const [newTaskTitle, setNewTaskTitle] = useState('');
  const [newTaskLabel, setNewTaskLabel] = useState('CORE');

  const handleAddTask = (e) => {
    e.preventDefault();
    if (!newTaskTitle) return;

    if (tasksSource === 'api') {
      apiCall('POST', '/api/entity', {
        module: 'tasks',
        type: 'task',
        title: newTaskTitle,
        status: 'DRAFT',
        attrs: { label: newTaskLabel },
      }).then(({ ok, data, offline }) => {
        if (ok && !offline) {
          saveTasks([entityToTask(data), ...tasks]);
          // Semantic event on top of the backend's generic entity.created -
          // the Tasks manifest declares task.created (docs/MODULES.md §2.2).
          apiCall('POST', '/api/event', { type: 'task.created', actor: 'user', entity_id: data.id, attrs: { title: data.title } });
        }
      });
    } else {
      const newTask = { id: Date.now(), title: newTaskTitle, status: 'DRAFT', label: newTaskLabel };
      saveTasks([newTask, ...tasks]);
    }
    setNewTaskTitle('');
  };

  // Optimistic move: update the board immediately, persist via PATCH, and
  // roll back to the prior status if the server rejects it.
  const moveTask = (taskId, nextStatus) => {
    const prevTasks = tasks;
    const updated = tasks.map(t => t.id === taskId ? { ...t, status: nextStatus } : t);
    saveTasks(updated);

    if (tasksSource !== 'api') return;
    apiCall('PATCH', `/api/entity/${taskId}`, { status: nextStatus }).then(({ ok, offline }) => {
      if (!ok || offline) { saveTasks(prevTasks); return; }
      if (nextStatus === 'COMPLETED') {
        apiCall('POST', '/api/event', { type: 'task.completed', actor: 'user', entity_id: taskId });
      }
    });
  };

  // Social draft approve/deny: PATCH the real entity status (if api-backed)
  // and always append the audit event - the gated publish action itself is
  // out of scope here (no real publish tool is registered anywhere per
  // docs/SECURITY.md), this only records the human decision honestly.
  const decideDraft = (draftId, decision) => {
    const draft = socialDrafts.find((d) => d.id === draftId);
    if (!draft) return;
    const nextStatus = decision === 'approve' ? 'published' : 'rejected';
    const prev = socialDrafts;
    setSocialDrafts(socialDrafts.map((d) => (d.id === draftId ? { ...d, status: nextStatus } : d)));

    if (socialDraftsSource === 'api') {
      apiCall('PATCH', `/api/entity/${draftId}`, { status: nextStatus }).then(({ ok, offline }) => {
        if (!ok || offline) setSocialDrafts(prev);
      });
    }

    // Append-only audit trail of the human-gated decision - never an edit/delete.
    apiCall('POST', '/api/event', {
      type: decision === 'approve' ? 'post.published' : 'post.rejected',
      actor: 'social_module',
      entity_id: socialDraftsSource === 'api' ? draftId : undefined,
      attrs: { text: draft.text, platform: draft.platform, account: draft.account },
    });
  };

  // Figma/Higgsfield Asset Generator simulation
  const handleGenerateAsset = () => {
    setIsRunningGen(true);
    setTimeout(() => {
      const newAsset = {
        name: promptInput.toLowerCase().replace(/\s+/g, '_') + '.svg',
        size: '18 KB',
        color: 'bg-amber-100',
        label: 'DESIGN'
      };
      const updated = [newAsset, ...assets];
      setAssets(updated);
      localStorage.setItem('life_os_assets', JSON.stringify(updated));
      setIsRunningGen(false);
    }, 1500);
  };

  // Generic entities for modules that render the generic views (no bespoke UI).
  // In the real system these are rows in the `entities` table; here they are
  // mock rows so the views are never empty.
  const GENERIC = {
    projects: {
      label: 'STATUS',
      rows: [
        { title: 'DeObfusca-AI v2', meta: 'GNN + Z3 symbolic exec', status: 'IN_PROGRESS' },
        { title: 'Life OS self-extension', meta: 'Claude Agent SDK builder', status: 'REVIEW' },
        { title: 'NeuralYul superoptimizer', meta: 'RL-guided EVM passes', status: 'DRAFT' },
        { title: 'Mirrorscope debugger', meta: 'eBPF time-travel', status: 'COMPLETED' },
      ],
    },
    trading: {
      label: 'SIGNAL',
      rows: [
        { title: 'NIFTY swing model', meta: 'order-flow + microstructure', status: 'IN_PROGRESS' },
        { title: 'HDFC GTT ladder', meta: 'read-only, human-executed', status: 'REVIEW' },
        { title: 'Kite WS ingest', meta: 'live tick → features', status: 'COMPLETED' },
      ],
    },
    marketing: {
      label: 'STAGE',
      rows: [
        { title: 'Launch announcement', meta: 'multi-channel, draft→approve', status: 'DRAFT' },
        { title: 'Newsletter Q3', meta: 'segmented send', status: 'IN_PROGRESS' },
        { title: 'Case study: self-extension', meta: 'long-form', status: 'REVIEW' },
      ],
    },
  };

  return (
    <div className="flex flex-col gap-8">
      {/* Module Tabs Selector */}
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-8 gap-3">
        {[
          { id: 'tasks', label: 'Tasks', icon: CheckSquare },
          { id: 'projects', label: 'Projects', icon: FolderKanban },
          { id: 'trading', label: 'Trading', icon: TrendingUp },
          { id: 'social', label: 'Social', icon: MessageSquare },
          { id: 'marketing', label: 'Marketing', icon: Megaphone },
          { id: 'design', label: 'Design', icon: Palette },
        ].concat(installedModules).map((mod) => (
          <button
            key={mod.id}
            onClick={() => {
              setActiveModule(mod.id);
              if (mod.id === 'tasks') setViewStyle('board');
              else if (mod.id === 'trading') setViewStyle('list');
              else if (mod.id === 'design') setViewStyle('gallery');
              else if (mod.id === 'health') setViewStyle('board');
              else setViewStyle('list');
            }}
            className={`neo-btn py-3 px-2 flex flex-col items-center gap-2 ${
              activeModule === mod.id ? 'bg-neo-yellow' : 'bg-neo-surface'
            }`}
          >
            <div className="w-8 h-8 rounded-none border-2 border-neo-border flex items-center justify-center bg-neo-surface">
              {mod.icon ? <mod.icon size={16} /> : <Heart size={16} />}
            </div>
            <span className="neo-label-sm text-[11px] font-bold truncate max-w-full block">{mod.label}</span>
          </button>
        ))}
      </div>

      {/* View Style Toolbar */}
      <div className="neo-surface neo-border p-4 bg-neo-surface flex flex-wrap gap-2 items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="neo-label-sm text-neo-text-muted text-[10px]">DECLARATIVE VIEW KIND:</span>
        </div>
        <div className="flex flex-wrap gap-2">
          {[
            { id: 'board', label: 'Kanban Board', icon: Grid },
            { id: 'list', label: 'Table / List', icon: List },
            { id: 'calendar', label: 'Calendar', icon: Calendar },
            { id: 'graph', label: 'Cytoscape Graph', icon: GitBranch },
            { id: 'gallery', label: 'Asset Gallery', icon: Palette },
            { id: 'timeline', label: 'Itinerary Timeline', icon: Clock },
            { id: 'map', label: 'Travel Map', icon: MapPin },
          ].map((style) => (
            <button
              key={style.id}
              onClick={() => setViewStyle(style.id)}
              className={`neo-btn py-1 px-2.5 text-xs font-mono flex items-center gap-1.5 ${
                viewStyle === style.id ? 'bg-neo-yellow' : 'bg-neo-surface'
              }`}
            >
              <style.icon size={12} />
              {style.label}
            </button>
          ))}
        </div>
      </div>

      {/* Main Module Content Panel */}
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-surface min-h-[480px] flex flex-col">
        
        {/* Module Header */}
        <div className="flex justify-between items-center border-b-4 border-neo-border pb-4 mb-6">
          <div>
            <span className="neo-chip neo-chip--active text-[10px] mb-2 uppercase">
              {activeModule === 'health' ? 'Self-Built Extension' : 'Core Seed Module'}
            </span>
            <h3 className="neo-title-md uppercase">{activeModule} Playground</h3>
          </div>
          <div className="flex items-center gap-2">
            <AIEdit prefill={`In the ${activeModule} module, `} label="Modify module with AI" />
            <span className="neo-tag bg-neo-surface-muted text-[10px]">`modules/{activeModule}/module.js`</span>
          </div>
        </div>

        {/* 1. TASKS / PRODUCTIVITY MODULE VIEW - rendered by the generic Board
            renderer (core/renderers/GenericBoard) driven by a display config,
            not bespoke per-module JSX. */}
        {activeModule === 'tasks' && viewStyle === 'board' && (
          <div className="flex-1 flex flex-col gap-6">
            <GenericBoard
              entities={tasks}
              setEntities={saveTasks}
              columns={['DRAFT', 'IN_PROGRESS', 'REVIEW', 'COMPLETED']}
              display={{ title: 'title', badge: 'label' }}
              onMove={tasksSource === 'api' ? undefined : (task, nextStatus) => moveTask(task.id, nextStatus)}
            />

            {/* Quick Add Form */}
            <div className="p-4 bg-neo-bg neo-border">
              <span className="neo-label-sm font-bold block mb-2">Insert New Task Card</span>
              <form onSubmit={handleAddTask} className="flex flex-wrap gap-3">
                <input 
                  type="text" 
                  placeholder="Task Description..."
                  value={newTaskTitle}
                  onChange={(e) => setNewTaskTitle(e.target.value)}
                  className="p-2 neo-border bg-neo-surface text-xs flex-1"
                />
                <select 
                  value={newTaskLabel}
                  onChange={(e) => setNewTaskLabel(e.target.value)}
                  className="p-2 neo-border bg-neo-surface text-xs font-bold"
                >
                  <option value="CORE">CORE</option>
                  <option value="DEVOPS">DEVOPS</option>
                  <option value="TRADING">TRADING</option>
                  <option value="MARKETING">MARKETING</option>
                </select>
                <button type="submit" className="neo-btn bg-neo-yellow py-2 px-4 text-xs font-bold">
                  Add to Kanban Board
                </button>
              </form>
            </div>
          </div>
        )}

        {/* 3. SOCIAL MODULE VIEW */}
        {activeModule === 'social' && (
          <div className="flex-1 flex flex-col gap-6">
            <h4 className="neo-label-md">Pending Multi-Account Social Drafts</h4>
            <p className="text-xs text-neo-text-muted">
              All state change or publishing tool executions require approval. Approve drafts here to publish them.
            </p>

            <div className="flex flex-col gap-4">
              {socialDrafts.map((draft) => {
                const pending = draft.status === 'drafted' || !draft.status;
                return (
                  <div
                    key={draft.id}
                    className={`p-4 neo-border flex flex-col md:flex-row justify-between items-start md:items-center gap-4 ${
                      pending ? 'bg-neo-red/10 border-neo-red' : 'bg-neo-bg'
                    }`}
                  >
                    <div className="max-w-2xl">
                      <div className="flex items-center gap-2 mb-1">
                        <span className="neo-chip py-0.5 text-[8px] font-mono">{draft.platform}</span>
                        <span className="neo-tag text-[9px] font-bold">{draft.account}</span>
                        {pending && <span className="neo-tag text-[8px] font-mono bg-neo-red text-white">PENDING APPROVAL</span>}
                      </div>
                      <p className="text-xs italic text-neo-text-muted font-semibold mt-1">
                        "{draft.text}"
                      </p>
                    </div>

                    <div className="flex gap-2 shrink-0">
                      {draft.status === 'published' && (
                        <span className="neo-chip neo-chip--completed text-[9px]">✅ PUBLISHED</span>
                      )}
                      {draft.status === 'rejected' && (
                        <span className="neo-chip neo-chip--overdue text-[9px]">✖ REJECTED</span>
                      )}
                      {pending && (
                        <>
                          <button
                            onClick={() => decideDraft(draft.id, 'deny')}
                            className="neo-btn bg-neo-surface py-1.5 px-3 text-xs font-bold"
                          >
                            DENY
                          </button>
                          <button
                            onClick={() => decideDraft(draft.id, 'approve')}
                            className="neo-btn bg-neo-yellow py-1.5 px-3 text-xs font-bold"
                          >
                            APPROVE & PUBLISH 🔒
                          </button>
                        </>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* 4. DESIGN MODULE VIEW */}
        {activeModule === 'design' && (
          <div className="flex-1 flex flex-col gap-6">
            {/* Higgsfield & Figma Asset Generator */}
            <div className="p-4 bg-neo-bg neo-border">
              <span className="neo-label-sm font-bold block mb-2 flex items-center gap-2">
                <Palette size={14} />
                Higgsfield AI Image/Vector Generator Simulator
              </span>
              <div className="flex gap-3">
                <input 
                  type="text" 
                  value={promptInput}
                  onChange={(e) => setPromptInput(e.target.value)}
                  className="p-2 neo-border bg-neo-surface text-xs flex-1 font-mono"
                />
                <button 
                  onClick={handleGenerateAsset}
                  disabled={isGenerating}
                  className="neo-btn bg-neo-mint py-2 px-4 text-xs font-bold"
                >
                  {isGenerating ? "GENERATING..." : "GENERATE VECTOR"}
                </button>
              </div>
            </div>

            {/* Gallery Section */}
            <h4 className="neo-label-md mt-4">Current Asset Gallery</h4>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              {assets.map((asset, idx) => (
                <div key={idx} className="neo-border neo-shadow bg-neo-surface p-3 flex flex-col justify-between min-h-[160px]">
                  <div className={`w-full h-24 ${asset.color || 'bg-indigo-50'} border-2 border-neo-border flex items-center justify-center font-mono text-xs font-bold text-center p-2`}>
                    {asset.name.split('.').pop().toUpperCase()}
                  </div>
                  <div className="mt-2">
                    <span className="neo-tag text-[8px]">{asset.label}</span>
                    <span className="neo-label-md text-xs block mt-1 truncate">{asset.name}</span>
                    <span className="text-[10px] text-neo-text-muted font-mono">{asset.size}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* 5. HEALTH MODULE (DYNAMIC) VIEW */}
        {activeModule === 'health' && (
          <div className="flex-1 flex flex-col gap-6">
            <p className="text-xs text-neo-text-muted">
              This module was generated dynamically on the Mac host using the Claude Agent SDK and hot-loaded into memory!
            </p>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              <div className="p-4 bg-neo-surface-high neo-border text-center">
                <span className="neo-label-sm text-neo-text-muted block mb-1">DAILY STEP TARGET</span>
                <span className="neo-title-xl block text-emerald-600 text-4xl font-bold">10,000 steps</span>
                <span className="text-[10px] block mt-1">Logged today: 8,432 (84%)</span>
              </div>
              <div className="p-4 bg-neo-surface-high neo-border text-center">
                <span className="neo-label-sm text-neo-text-muted block mb-1">WATER TARGET</span>
                <span className="neo-title-xl block text-neo-blue text-4xl font-bold">3.5 Litres</span>
                <span className="text-[10px] block mt-1">Logged today: 2.0L (57%)</span>
              </div>
              <div className="p-4 bg-neo-surface-high neo-border text-center">
                <span className="neo-label-sm text-neo-text-muted block mb-1">CALORIE BURN</span>
                <span className="neo-title-xl block text-amber-500 text-4xl font-bold">2,400 kcal</span>
                <span className="text-[10px] block mt-1">Logged today: 1,820 (75%)</span>
              </div>
            </div>
          </div>
        )}

        {/* GENERIC-ENTITY MODULES (projects / trading / marketing): the module
            manifest declares no bespoke UI, so the core renders generic views
            (board or list) over its entity rows. */}
        {GENERIC[activeModule] && (
          <div className="flex-1 flex flex-col gap-4">
            <p className="text-xs text-neo-text-muted">
              This module declares no custom UI - the core renders generic views (board / list / calendar) directly over its <code>entities</code> rows. Switch the view kind above.
            </p>

            {/* Board view */}
            {(viewStyle === 'board' || viewStyle === 'graph' || viewStyle === 'gallery') && (
              <div className="grid grid-cols-1 md:grid-cols-4 gap-4 flex-1">
                {['DRAFT', 'IN_PROGRESS', 'REVIEW', 'COMPLETED'].map((col) => (
                  <div key={col} className="p-4 bg-neo-bg neo-border flex flex-col gap-3">
                    <div className="border-b-2 border-neo-border pb-2 flex justify-between items-center">
                      <span className="neo-label-sm font-bold text-xs">{col}</span>
                      <span className="text-[10px] font-mono bg-neo-surface px-1.5 neo-border">
                        {GENERIC[activeModule].rows.filter((r) => r.status === col).length}
                      </span>
                    </div>
                    <div className="flex flex-col gap-3">
                      {GENERIC[activeModule].rows.filter((r) => r.status === col).map((r, i) => (
                        <div key={i} className="p-3 bg-neo-surface neo-border flex flex-col gap-1">
                          <p className="text-xs font-bold leading-tight">{r.title}</p>
                          <span className="text-[10px] text-neo-text-muted">{r.meta}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            )}

            {/* List / table view */}
            {(viewStyle === 'list' || viewStyle === 'calendar') && (
              <div className="neo-border overflow-hidden">
                <table className="w-full text-xs">
                  <thead className="bg-neo-surface-high">
                    <tr className="text-left">
                      <th className="p-2.5 neo-label-sm text-[10px]">Title</th>
                      <th className="p-2.5 neo-label-sm text-[10px]">Detail</th>
                      <th className="p-2.5 neo-label-sm text-[10px]">{GENERIC[activeModule].label}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {GENERIC[activeModule].rows.map((r, i) => (
                      <tr key={i} className="border-t-2 border-neo-border">
                        <td className="p-2.5 font-bold text-neo-text">{r.title}</td>
                        <td className="p-2.5 text-neo-text-muted">{r.meta}</td>
                        <td className="p-2.5"><span className="neo-tag text-[9px]">{r.status}</span></td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        )}

        {/* Generic viewkinds rendering (board, list, calendar, graph, gallery, timeline, map) fallback */}
        {viewStyle === 'timeline' && GENERIC[activeModule] && (
          <div className="mt-8 border-t-2 border-neo-border border-dashed pt-6 flex-1 flex flex-col gap-4">
            <h4 className="neo-label-md">Trip Itinerary & Event Timeline</h4>
            <div className="flex flex-col gap-4 relative pl-6 before:absolute before:left-2 before:top-2 before:bottom-2 before:w-1 before:bg-neo-border">
              {[
                { time: '09:00 AM', title: 'Flight departures (LEG_FLIGHT)', desc: 'AI auto-blocked schedule blocks on calendar.' },
                { time: '02:00 PM', title: 'Hotel Check-in confirmation (LEG_BOOKING)', desc: 'Nango credentials fetched confirmation code.' },
                { time: '04:30 PM', title: 'Client presentation (LEG_MEETING)', desc: 'Topic: Local offline vector storage features.' }
              ].map((item, idx) => (
                <div key={idx} className="relative bg-neo-surface p-4 border-2 border-neo-border shadow-sm text-xs">
                  <div className="absolute -left-7 top-4 w-3 h-3 bg-neo-surface border-2 border-neo-border rounded-full" />
                  <span className="neo-chip neo-chip--active py-0.5 text-[9px] mb-2">{item.time}</span>
                  <h5 className="font-bold text-xs">{item.title}</h5>
                  <p className="text-[11px] text-neo-text-muted mt-1">{item.desc}</p>
                </div>
              ))}
            </div>
          </div>
        )}

        {viewStyle === 'map' && GENERIC[activeModule] && (
          <div className="mt-8 border-t-2 border-neo-border border-dashed pt-6 flex-1 flex flex-col gap-4">
            <h4 className="neo-label-md">Trip Location Mapping</h4>
            <div className="p-8 bg-zinc-950 border-4 border-neo-border text-center text-zinc-400 font-mono text-xs flex flex-col justify-center items-center gap-3 min-h-[300px]">
              <Compass className="text-neo-yellow animate-pulse" size={48} />
              <div>
                <p className="text-white font-bold">Interactive Geolocation Mapping API</p>
                <p className="text-[10px] text-zinc-500 mt-1">LATITUDE: 12.9716° N / LONGITUDE: 77.5946° E</p>
              </div>
              <div className="flex gap-2">
                <span className="neo-chip bg-neo-surface text-neo-text text-[9px]">LEG_1: BLR AIRPORT</span>
                <span className="neo-chip bg-neo-surface text-neo-text text-[9px]">LEG_2: DOWNTOWN HOTEL</span>
              </div>
            </div>
          </div>
        )}

      </div>
    </div>
  );
}
