import React, { useState, useEffect } from 'react';
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
  Heart
} from 'lucide-react';

export default function ModulesView() {
  const [activeModule, setActiveModule] = useState('learning');
  const [viewStyle, setViewStyle] = useState('graph'); // board, list, calendar, graph, gallery, timeline, map

  // Flashcard mock state for learning module
  const [flashcardSide, setFlashcardSide] = useState('question');
  const [currentFlashcardIdx, setCurrentFlashcardIdx] = useState(0);
  const flashcards = [
    { q: "What is FastCDC?", a: "Content-Defined Chunking algorithm that uses rolling hashes to find deduplication boundaries in media." },
    { q: "What is Turso Sync bidirectional mode?", a: "embedded-replica sync which synchronizes writes to the canonical primary database while offline is set." },
    { q: "What is the PreToolUse hook security guard?", a: "A custom programmatic hook that intercepts file operations, restricting write access solely to modules/." },
  ];

  const [score, setScore] = useState({ reviewDue: 3, mastered: 12 });

  // Task Board state (with localStorage persistency)
  const [tasks, setTasks] = useState([
    { id: 1, title: 'Map ENCODE GraphQL API schema', status: 'IN_PROGRESS', label: 'GENETICS' },
    { id: 2, title: 'Write SQLite FTS5 index trigger', status: 'REVIEW', label: 'CORE' },
    { id: 3, title: 'Setup Nango instance on fly.io', status: 'COMPLETED', label: 'DEVOPS' },
    { id: 4, title: 'Verify broker-guard closed bounds', status: 'OVERDUE', label: 'TRADING' },
  ]);

  // Social account connection draft status
  const [socialDrafts, setSocialDrafts] = useState([
    { id: 1, platform: 'X / Twitter', account: '@life_os_dev', text: 'Exciting news! Life OS self-extension validation pipeline is officially 100% locally sandboxed. Headless Playwright assertions prevent build leaks.', status: 'PENDING' },
    { id: 2, platform: 'Instagram', account: 'life_os_studio', text: 'Behind the scenes: Spinning up custom connectors using self-hosted Nango OAuth vault.', status: 'DRAFT' }
  ]);

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
  }, []);

  const saveTasks = (newTasks) => {
    setTasks(newTasks);
    localStorage.setItem('life_os_tasks', JSON.stringify(newTasks));
  };

  // Quick form for adding task
  const [newTaskTitle, setNewTaskTitle] = useState('');
  const [newTaskLabel, setNewTaskLabel] = useState('CORE');

  const handleAddTask = (e) => {
    e.preventDefault();
    if (!newTaskTitle) return;
    const newTask = {
      id: Date.now(),
      title: newTaskTitle,
      status: 'DRAFT',
      label: newTaskLabel
    };
    const updated = [newTask, ...tasks];
    saveTasks(updated);
    setNewTaskTitle('');
  };

  const moveTask = (taskId, nextStatus) => {
    const updated = tasks.map(t => t.id === taskId ? { ...t, status: nextStatus } : t);
    saveTasks(updated);
  };

  // Spaced Repetition Mastery Quiz Handlers
  const handleScoreQuiz = (knows) => {
    if (knows) {
      setScore(prev => ({ ...prev, mastered: prev.mastered + 1 }));
    } else {
      setScore(prev => ({ ...prev, reviewDue: prev.reviewDue + 1 }));
    }
    setFlashcardSide('question');
    setCurrentFlashcardIdx((currentFlashcardIdx + 1) % flashcards.length);
  };

  // Social Draft Approval
  const handleApproveDraft = (draftId) => {
    const updated = socialDrafts.map(d => d.id === draftId ? { ...d, status: 'PUBLISHED' } : d);
    setSocialDrafts(updated);
    
    // Log as a custom published event in localStorage events to mirror system action
    const customEvents = JSON.parse(localStorage.getItem('life_os_custom_events') || '[]');
    customEvents.unshift({
      id: "ev_" + Math.random().toString(36).substring(2, 9),
      ts: Date.now(),
      type: "post.published",
      actor: "social_module",
      attrs: { text: socialDrafts.find(d => d.id === draftId).text }
    });
    localStorage.setItem('life_os_custom_events', JSON.stringify(customEvents));

    alert("Draft Approved! Simulating Nango Proxy publishing to X API...");
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
      alert(`Asset generated and saved in lifeos-vcs content-addressed repository!`);
    }, 1500);
  };

  return (
    <div className="flex flex-col gap-8">
      {/* Module Tabs Selector */}
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-8 gap-3">
        {[
          { id: 'learning', label: 'Learning', icon: GraduationCap },
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
              if (mod.id === 'learning') setViewStyle('graph');
              else if (mod.id === 'tasks') setViewStyle('board');
              else if (mod.id === 'trading') setViewStyle('list');
              else if (mod.id === 'design') setViewStyle('gallery');
              else if (mod.id === 'health') setViewStyle('board');
              else setViewStyle('list');
            }}
            className={`neo-btn py-3 px-2 flex flex-col items-center gap-2 ${
              activeModule === mod.id ? 'bg-[var(--neo-yellow)]' : 'bg-white'
            }`}
          >
            <div className="w-8 h-8 rounded-none border-2 border-black flex items-center justify-center bg-white">
              {mod.icon ? <mod.icon size={16} /> : <Heart size={16} />}
            </div>
            <span className="neo-label-sm text-[11px] font-bold truncate max-w-full block">{mod.label}</span>
          </button>
        ))}
      </div>

      {/* View Style Toolbar */}
      <div className="neo-surface neo-border p-4 bg-white flex flex-wrap gap-2 items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="neo-label-sm text-[var(--neo-text-muted)] text-[10px]">DECLARATIVE VIEW KIND:</span>
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
                viewStyle === style.id ? 'bg-[var(--neo-yellow)]' : 'bg-white'
              }`}
            >
              <style.icon size={12} />
              {style.label}
            </button>
          ))}
        </div>
      </div>

      {/* Main Module Content Panel */}
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-white min-h-[480px] flex flex-col">
        
        {/* Module Header */}
        <div className="flex justify-between items-center border-b-4 border-[var(--neo-border)] pb-4 mb-6">
          <div>
            <span className="neo-chip neo-chip--active text-[10px] mb-2 uppercase">
              {activeModule === 'health' ? 'Self-Built Extension' : 'Core Seed Module'}
            </span>
            <h3 className="neo-title-md uppercase">{activeModule} Playground</h3>
          </div>
          <div className="flex items-center gap-2">
            <span className="neo-tag bg-[var(--neo-surface-muted)] text-[10px]">`modules/{activeModule}/module.js`</span>
          </div>
        </div>

        {/* 1. LEARNING MODULE VIEW */}
        {activeModule === 'learning' && (
          <div className="flex-1 flex flex-col gap-6">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              {/* Mastery Stats */}
              <div className="p-4 bg-[var(--neo-bg)] neo-border flex flex-col items-center justify-center text-center">
                <span className="neo-label-sm text-[var(--neo-text-muted)] block mb-1">TOTAL MASTERED TOPICS</span>
                <span className="neo-title-xl block text-emerald-600 text-5xl font-black">{score.mastered}</span>
              </div>
              <div className="p-4 bg-[var(--neo-bg)] neo-border flex flex-col items-center justify-center text-center">
                <span className="neo-label-sm text-[var(--neo-text-muted)] block mb-1">SPACED REPETITIONS DUE</span>
                <span className="neo-title-xl block text-amber-500 text-5xl font-black">{score.reviewDue}</span>
              </div>

              {/* Flashcard Examiner Simulator */}
              <div className="p-4 bg-white neo-border flex flex-col gap-3">
                <span className="neo-label-sm font-bold text-xs text-[var(--neo-blue)]">Spaced Repetition Review</span>
                
                <div 
                  onClick={() => setFlashcardSide(flashcardSide === 'question' ? 'answer' : 'question')}
                  className="p-4 min-h-[100px] border-2 border-black border-dashed flex flex-col justify-center items-center text-center cursor-pointer hover:bg-slate-50 transition-colors"
                >
                  <span className="text-[10px] text-gray-400 font-mono mb-2 uppercase">
                    Click to flip • {flashcardSide.toUpperCase()}
                  </span>
                  <p className="text-xs font-bold leading-tight">
                    {flashcardSide === 'question' ? flashcards[currentFlashcardIdx].q : flashcards[currentFlashcardIdx].a}
                  </p>
                </div>

                <div className="flex gap-2">
                  <button 
                    onClick={() => handleScoreQuiz(true)}
                    className="flex-1 py-1 px-3 border border-black bg-[var(--neo-mint)] font-mono text-[10px] font-bold"
                  >
                    KNOW WELL (+1)
                  </button>
                  <button 
                    onClick={() => handleScoreQuiz(false)}
                    className="flex-1 py-1 px-3 border border-black bg-[var(--neo-red)] text-white font-mono text-[10px] font-bold"
                  >
                    RE-STUDY
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* 2. TASKS / PRODUCTIVITY MODULE VIEW */}
        {activeModule === 'tasks' && viewStyle === 'board' && (
          <div className="flex-1 flex flex-col gap-6">
            <div className="grid grid-cols-1 md:grid-cols-4 gap-4 flex-1">
              {['DRAFT', 'IN_PROGRESS', 'REVIEW', 'COMPLETED'].map((col) => (
                <div key={col} className="p-4 bg-[var(--neo-bg)] neo-border flex flex-col gap-3">
                  <div className="border-b-2 border-black pb-2 mb-1 flex justify-between items-center">
                    <span className="neo-label-sm font-bold text-xs">{col}</span>
                    <span className="text-[10px] font-mono bg-white px-1.5 neo-border">
                      {tasks.filter(t => t.status === col).length}
                    </span>
                  </div>
                  
                  <div className="flex flex-col gap-3 overflow-y-auto max-h-[250px]">
                    {tasks.filter(t => t.status === col).map((task) => (
                      <div key={task.id} className="p-3 bg-white neo-border neo-shadow-sm hover:scale-[1.02] transition-all flex flex-col gap-2">
                        <div>
                          <span className="neo-tag text-[8px] mb-2">{task.label}</span>
                          <p className="text-xs font-bold leading-tight mt-1">{task.title}</p>
                        </div>
                        <div className="flex gap-1.5 pt-2 border-t border-dashed">
                          {col !== 'DRAFT' && (
                            <button 
                              onClick={() => {
                                const phases = ['DRAFT', 'IN_PROGRESS', 'REVIEW', 'COMPLETED'];
                                const idx = phases.indexOf(col);
                                moveTask(task.id, phases[idx - 1]);
                              }}
                              className="text-[9px] font-bold font-mono text-gray-500 hover:underline"
                            >
                              ← Prev
                            </button>
                          )}
                          {col !== 'COMPLETED' && (
                            <button 
                              onClick={() => {
                                const phases = ['DRAFT', 'IN_PROGRESS', 'REVIEW', 'COMPLETED'];
                                const idx = phases.indexOf(col);
                                moveTask(task.id, phases[idx + 1]);
                              }}
                              className="text-[9px] font-bold font-mono text-[var(--neo-blue)] hover:underline ml-auto"
                            >
                              Next →
                            </button>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>

            {/* Quick Add Form */}
            <div className="p-4 bg-[var(--neo-bg)] neo-border">
              <span className="neo-label-sm font-bold block mb-2">Insert New Task Card</span>
              <form onSubmit={handleAddTask} className="flex flex-wrap gap-3">
                <input 
                  type="text" 
                  placeholder="Task Description..."
                  value={newTaskTitle}
                  onChange={(e) => setNewTaskTitle(e.target.value)}
                  className="p-2 neo-border bg-white text-xs flex-1"
                />
                <select 
                  value={newTaskLabel}
                  onChange={(e) => setNewTaskLabel(e.target.value)}
                  className="p-2 neo-border bg-white text-xs font-bold"
                >
                  <option value="CORE">CORE</option>
                  <option value="DEVOPS">DEVOPS</option>
                  <option value="TRADING">TRADING</option>
                  <option value="MARKETING">MARKETING</option>
                </select>
                <button type="submit" className="neo-btn bg-[var(--neo-yellow)] py-2 px-4 text-xs font-bold">
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
            <p className="text-xs text-[var(--neo-text-muted)]">
              All state change or publishing tool executions require approval. Approve drafts here to publish them.
            </p>

            <div className="flex flex-col gap-4">
              {socialDrafts.map((draft) => (
                <div key={draft.id} className="p-4 bg-[var(--neo-bg)] neo-border flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
                  <div className="max-w-2xl">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="neo-chip py-0.5 text-[8px] font-mono">{draft.platform}</span>
                      <span className="neo-tag text-[9px] font-bold">{draft.account}</span>
                    </div>
                    <p className="text-xs italic text-[var(--neo-text-muted)] font-semibold mt-1">
                      "{draft.text}"
                    </p>
                  </div>

                  <div>
                    {draft.status === 'PUBLISHED' ? (
                      <span className="neo-chip neo-chip--completed text-[9px]">✅ MOUNTED / PUBLISHED</span>
                    ) : (
                      <button 
                        onClick={() => handleApproveDraft(draft.id)}
                        className="neo-btn bg-[var(--neo-yellow)] py-1.5 px-3 text-xs font-bold"
                      >
                        APPROVE & PUBLISH 🔒
                      </button>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* 4. DESIGN MODULE VIEW */}
        {activeModule === 'design' && (
          <div className="flex-1 flex flex-col gap-6">
            {/* Higgsfield & Figma Asset Generator */}
            <div className="p-4 bg-[var(--neo-bg)] neo-border">
              <span className="neo-label-sm font-bold block mb-2 flex items-center gap-2">
                <Palette size={14} />
                Higgsfield AI Image/Vector Generator Simulator
              </span>
              <div className="flex gap-3">
                <input 
                  type="text" 
                  value={promptInput}
                  onChange={(e) => setPromptInput(e.target.value)}
                  className="p-2 neo-border bg-white text-xs flex-1 font-mono"
                />
                <button 
                  onClick={handleGenerateAsset}
                  disabled={isGenerating}
                  className="neo-btn bg-[var(--neo-mint)] py-2 px-4 text-xs font-bold"
                >
                  {isGenerating ? "GENERATING..." : "GENERATE VECTOR"}
                </button>
              </div>
            </div>

            {/* Gallery Section */}
            <h4 className="neo-label-md mt-4">Current Asset Gallery</h4>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              {assets.map((asset, idx) => (
                <div key={idx} className="neo-border neo-shadow bg-white p-3 flex flex-col justify-between min-h-[160px]">
                  <div className={`w-full h-24 ${asset.color || 'bg-indigo-50'} border-2 border-black flex items-center justify-center font-mono text-xs font-bold text-center p-2`}>
                    {asset.name.split('.').pop().toUpperCase()}
                  </div>
                  <div className="mt-2">
                    <span className="neo-tag text-[8px]">{asset.label}</span>
                    <span className="neo-label-md text-xs block mt-1 truncate">{asset.name}</span>
                    <span className="text-[10px] text-[var(--neo-text-muted)] font-mono">{asset.size}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* 5. HEALTH MODULE (DYNAMIC) VIEW */}
        {activeModule === 'health' && (
          <div className="flex-1 flex flex-col gap-6">
            <p className="text-xs text-[var(--neo-text-muted)]">
              This module was generated dynamically on the Mac host using the Claude Agent SDK and hot-loaded into memory!
            </p>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              <div className="p-4 bg-emerald-50 neo-border text-center">
                <span className="neo-label-sm text-[var(--neo-text-muted)] block mb-1">DAILY STEP TARGET</span>
                <span className="neo-title-xl block text-emerald-600 text-4xl font-bold">10,000 steps</span>
                <span className="text-[10px] block mt-1">Logged today: 8,432 (84%)</span>
              </div>
              <div className="p-4 bg-blue-50 neo-border text-center">
                <span className="neo-label-sm text-[var(--neo-text-muted)] block mb-1">WATER TARGET</span>
                <span className="neo-title-xl block text-blue-600 text-4xl font-bold">3.5 Litres</span>
                <span className="text-[10px] block mt-1">Logged today: 2.0L (57%)</span>
              </div>
              <div className="p-4 bg-amber-50 neo-border text-center">
                <span className="neo-label-sm text-[var(--neo-text-muted)] block mb-1">CALORIE BURN</span>
                <span className="neo-title-xl block text-amber-500 text-4xl font-bold">2,400 kcal</span>
                <span className="text-[10px] block mt-1">Logged today: 1,820 (75%)</span>
              </div>
            </div>
          </div>
        )}

        {/* FALLBACK INFO FOR NON-SEED MANIFEST RENDERING */}
        {activeModule !== 'learning' && activeModule !== 'tasks' && activeModule !== 'social' && activeModule !== 'design' && activeModule !== 'health' && (
          <div className="flex-1 flex flex-col gap-4 text-center justify-center py-12 bg-slate-50 border-2 border-black border-dashed">
            <Compass className="mx-auto text-gray-300" size={48} />
            <h4 className="neo-label-md text-xs uppercase">Module Renders Generic Views</h4>
            <p className="text-xs text-[var(--neo-text-muted)] max-w-md mx-auto">
              Declarative structures render generic templates for list, board, calendar, or gallery, pulling domain attrs directly from Turso database rows.
            </p>
          </div>
        )}

        {/* Generic viewkinds rendering (board, list, calendar, graph, gallery, timeline, map) fallback */}
        {viewStyle === 'timeline' && activeModule !== 'learning' && (
          <div className="mt-8 border-t-2 border-black border-dashed pt-6 flex-1 flex flex-col gap-4">
            <h4 className="neo-label-md">Trip Itinerary & Event Timeline</h4>
            <div className="flex flex-col gap-4 relative pl-6 before:absolute before:left-2 before:top-2 before:bottom-2 before:w-1 before:bg-black">
              {[
                { time: '09:00 AM', title: 'Flight departures (LEG_FLIGHT)', desc: 'AI auto-blocked schedule blocks on calendar.' },
                { time: '02:00 PM', title: 'Hotel Check-in confirmation (LEG_BOOKING)', desc: 'Nango credentials fetched confirmation code.' },
                { time: '04:30 PM', title: 'Client presentation (LEG_MEETING)', desc: 'Topic: Local offline vector storage features.' }
              ].map((item, idx) => (
                <div key={idx} className="relative bg-white p-4 border-2 border-black shadow-sm text-xs">
                  <div className="absolute -left-7 top-4 w-3 h-3 bg-white border-2 border-black rounded-full" />
                  <span className="neo-chip neo-chip--active py-0.5 text-[9px] mb-2">{item.time}</span>
                  <h5 className="font-bold text-xs">{item.title}</h5>
                  <p className="text-[11px] text-[var(--neo-text-muted)] mt-1">{item.desc}</p>
                </div>
              ))}
            </div>
          </div>
        )}

        {viewStyle === 'map' && activeModule !== 'learning' && (
          <div className="mt-8 border-t-2 border-black border-dashed pt-6 flex-1 flex flex-col gap-4">
            <h4 className="neo-label-md">Trip Location Mapping</h4>
            <div className="p-8 bg-zinc-950 border-4 border-black text-center text-zinc-400 font-mono text-xs flex flex-col justify-center items-center gap-3 min-h-[300px]">
              <Compass className="text-[var(--neo-yellow)] animate-pulse" size={48} />
              <div>
                <p className="text-white font-bold">Interactive Geolocation Mapping API</p>
                <p className="text-[10px] text-zinc-500 mt-1">LATITUDE: 12.9716° N / LONGITUDE: 77.5946° E</p>
              </div>
              <div className="flex gap-2">
                <span className="neo-chip bg-white text-black text-[9px]">LEG_1: BLR AIRPORT</span>
                <span className="neo-chip bg-white text-black text-[9px]">LEG_2: DOWNTOWN HOTEL</span>
              </div>
            </div>
          </div>
        )}

      </div>
    </div>
  );
}
