import React, { useEffect, useState, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { Search, ArrowRight, Sparkles, Plus } from 'lucide-react';
import { apiCall } from '../lib/api';

// Global Cmd-K command bar (core/command.js, generalized): fuzzy-routes free
// text to navigation, live entity search (GET /api/search), and quick
// actions across every module - the single entry point agent-mode's
// ActionPlan compiler (a separate, later issue) will hook into. Typing
// "> <anything>" routes straight to the AI Console as a hook point for that
// future compilation step, without this issue building the compiler itself.
const NAV_ACTIONS = [
  { id: 'nav-dashboard', label: 'Go to Dashboard', href: '/dashboard' },
  { id: 'nav-knowledge', label: 'Go to Knowledge', href: '/knowledge' },
  { id: 'nav-modules', label: 'Go to Modules', href: '/modules' },
  { id: 'nav-database', label: 'Go to Database', href: '/database' },
  { id: 'nav-graph', label: 'Go to Graph', href: '/graph' },
  { id: 'nav-harness', label: 'Go to Harness', href: '/harness' },
  { id: 'nav-storage', label: 'Go to Storage & VCS', href: '/storage' },
  { id: 'nav-integrations', label: 'Go to Integrations', href: '/integrations' },
  { id: 'nav-docs', label: 'Go to Docs', href: '/docs' },
  { id: 'nav-profile', label: 'Go to Profile', href: '/profile' },
];

function fuzzyMatch(label, query) {
  return label.toLowerCase().includes(query.toLowerCase());
}

export default function CommandBar() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [entityResults, setEntityResults] = useState([]);
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef(null);
  const navigate = useNavigate();

  useEffect(() => {
    const onKeyDown = (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setOpen((o) => !o);
      }
      if (e.key === 'Escape') setOpen(false);
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  useEffect(() => {
    if (open) setTimeout(() => inputRef.current?.focus(), 0);
    else { setQuery(''); setEntityResults([]); setActiveIndex(0); }
  }, [open]);

  useEffect(() => {
    if (!query.trim() || query.startsWith('>')) { setEntityResults([]); return; }
    const handle = setTimeout(() => {
      apiCall('GET', `/api/search?q=${encodeURIComponent(query)}&limit=8`).then(({ ok, data }) => {
        if (ok) setEntityResults(data?.results || []);
      });
    }, 200); // debounce - avoid a request per keystroke
    return () => clearTimeout(handle);
  }, [query]);

  const navActions = query.trim() && !query.startsWith('>')
    ? NAV_ACTIONS.filter((a) => fuzzyMatch(a.label, query))
    : (query.trim() ? [] : NAV_ACTIONS);

  const quickCreate = query.trim() && !query.startsWith('>')
    ? [{ id: 'create-task', label: `Create task: "${query.trim()}"`, icon: Plus }]
    : [];

  const items = query.startsWith('>')
    ? [{ id: 'ask-ai', label: `Ask AI: "${query.slice(1).trim()}"`, icon: Sparkles }]
    : [
        ...navActions.map((a) => ({ ...a, icon: ArrowRight })),
        ...entityResults.map((e) => ({ id: e.id, label: `${e.title || e.id} (${e.module}/${e.type})`, icon: Search, entity: e })),
        ...quickCreate,
      ];

  const runItem = async (item) => {
    if (!item) return;
    if (item.id === 'ask-ai') {
      window.dispatchEvent(new CustomEvent('lifeos:ai', { detail: { prefill: query.slice(1).trim() } }));
    } else if (item.id === 'create-task') {
      await apiCall('POST', '/api/entity', { module: 'tasks', type: 'task', title: query.trim(), status: 'DRAFT' });
      navigate('/modules');
    } else if (item.entity) {
      navigate(`/database?entity=${item.entity.id}`);
    } else if (item.href) {
      navigate(item.href);
    }
    setOpen(false);
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[200] flex items-start justify-center pt-[12vh]" onClick={() => setOpen(false)}>
      <div className="absolute inset-0 bg-black/40" />
      <div
        className="relative w-full max-w-xl neo-surface neo-border-thick neo-shadow-xl bg-neo-surface flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 p-3 border-b-2 border-neo-border">
          <Search size={16} className="text-neo-text-muted" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => { setQuery(e.target.value); setActiveIndex(0); }}
            onKeyDown={(e) => {
              if (e.key === 'ArrowDown') { e.preventDefault(); setActiveIndex((i) => Math.min(i + 1, items.length - 1)); }
              if (e.key === 'ArrowUp') { e.preventDefault(); setActiveIndex((i) => Math.max(i - 1, 0)); }
              if (e.key === 'Enter') { e.preventDefault(); runItem(items[activeIndex]); }
            }}
            placeholder="Search entities, jump to a page, or '> ask the AI anything'..."
            className="flex-1 bg-transparent text-sm focus:outline-none font-mono"
          />
          <span className="neo-tag text-[9px] font-mono">ESC</span>
        </div>
        <div className="max-h-80 overflow-y-auto p-2 flex flex-col gap-1">
          {items.length === 0 && <p className="text-xs text-neo-text-muted p-3">No matches.</p>}
          {items.map((item, i) => {
            const Icon = item.icon;
            return (
              <button
                key={item.id}
                onClick={() => runItem(item)}
                onMouseEnter={() => setActiveIndex(i)}
                className={`flex items-center gap-2 p-2 text-left text-xs font-semibold ${i === activeIndex ? 'bg-neo-yellow' : 'hover:bg-neo-surface-muted'}`}
              >
                <Icon size={14} /> {item.label}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
