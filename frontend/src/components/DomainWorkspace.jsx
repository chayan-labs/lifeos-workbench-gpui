import React, { useState, useEffect } from 'react';
import {
  BookOpen, ListTree, NotebookPen, Map, FileText, Hammer, ChevronRight, ChevronLeft,
  Check, Plus, Trash2, Save, ExternalLink
} from 'lucide-react';
import MarkdownRenderer from './MarkdownRenderer';
import Tabs from './ui/Tabs';
import AIButton from './ui/AIButton';
import EmptyState from './ui/EmptyState';
import {
  getNotes, setNotes, getPapers, addPaper, removePaper,
  getProjects, addProject, acceptProject,
} from '../lib/atlasStore';
import { generateRoadmap, summarizeNotes, summarizePaper, recommendProjects } from '../lib/ai';

// A full learning workspace for ONE domain. Tabs: Overview, Topics, Notes,
// Roadmap, Papers, Projects. AI is wired into every tab (summarize notes,
// generate roadmap, summarize a paper, recommend projects). Accepted projects
// are auto-added to the Repository/Modules via atlasStore.

const TABS = [
  { id: 'overview', label: 'Overview', icon: BookOpen },
  { id: 'topics', label: 'Topics', icon: ListTree },
  { id: 'notes', label: 'Notes', icon: NotebookPen },
  { id: 'roadmap', label: 'Roadmap', icon: Map },
  { id: 'papers', label: 'Papers', icon: FileText },
  { id: 'projects', label: 'Projects', icon: Hammer },
];

export default function DomainWorkspace({ domain, annotations, progress, onOpenTopic, onBack }) {
  const [tab, setTab] = useState('overview');

  // Notes
  const [notes, setNotesState] = useState('');
  const [noteSummary, setNoteSummary] = useState('');
  const [savingFlash, setSavingFlash] = useState(false);

  // Roadmap / papers / projects
  const [roadmap, setRoadmap] = useState(null);
  const [papers, setPapers] = useState([]);
  const [recommendText, setRecommendText] = useState('');
  const [projects, setProjects] = useState([]);

  // Paper composer
  const [paperTitle, setPaperTitle] = useState('');
  const [paperAbstract, setPaperAbstract] = useState('');

  const [loading, setLoading] = useState('');

  useEffect(() => {
    setNotesState(getNotes(domain.id));
    setPapers(getPapers(domain.id));
    setProjects(getProjects(domain.id));
    setRoadmap(null);
    setNoteSummary('');
    setTab('overview');
  }, [domain.id]);

  const saveNotes = () => {
    setNotes(domain.id, notes);
    setSavingFlash(true);
    setTimeout(() => setSavingFlash(false), 1500);
  };

  const runSummarizeNotes = async () => {
    setLoading('notes');
    const out = await summarizeNotes(domain.title, notes || '');
    setNoteSummary(out);
    setLoading('');
  };

  const runRoadmap = async () => {
    setLoading('roadmap');
    setRoadmap(await generateRoadmap(domain));
    setLoading('');
  };

  const runSummarizePaper = async () => {
    if (!paperTitle.trim()) return;
    setLoading('paper');
    const summary = await summarizePaper(paperTitle, paperAbstract);
    setPapers(addPaper(domain.id, { title: paperTitle, summary, url: '' }));
    setPaperTitle('');
    setPaperAbstract('');
    setLoading('');
  };

  const runRecommend = async () => {
    setLoading('projects');
    const out = await recommendProjects(domain);
    // Live backend path returns free-text recommendations ({ text }); the
    // offline mock returns structured ({ projects }). Render whichever we get
    // instead of silently dropping the AI text.
    if (out.text) {
      setRecommendText(out.text);
    } else {
      const list = out.projects || [];
      let next = projects;
      list.forEach((p) => { next = addProject(domain.id, p); });
      setProjects([...next]);
    }
    setLoading('');
  };

  const accept = (p) => setProjects([...acceptProject(domain.id, domain.title, p)]);

  return (
    <div className="flex flex-col gap-5">
      <button onClick={onBack} className="neo-btn self-start py-1 px-3 text-xs flex items-center gap-1">
        <ChevronLeft size={14} /> All Domains
      </button>

      <div className="p-5 neo-border-thick bg-[var(--neo-surface)]" style={{ borderTopColor: domain.color, borderTopWidth: '6px' }}>
        <h1 className="neo-title-md text-neo-text mb-3">{domain.icon} {domain.title}</h1>
        <Tabs tabs={TABS} active={tab} onChange={setTab} className="mb-5" />

        {/* OVERVIEW */}
        {tab === 'overview' && (
          <div className="text-sm text-neo-text-muted">
            <MarkdownRenderer content={domain.overview} />
            <div className="mt-4 text-[11px] text-neo-text-muted font-mono">
              {(domain.topics || []).length} topics · {papers.length} papers · {projects.length} projects
            </div>
          </div>
        )}

        {/* TOPICS */}
        {tab === 'topics' && (
          <div className="flex flex-col gap-2">
            {(domain.topics || []).length === 0 && (
              <EmptyState icon={ListTree} title="No topics yet" hint="Use the AI Console to add topics to this domain." />
            )}
            {(domain.topics || []).map((t) => {
              const topicAnns = annotations.filter((a) => a.topicId === t.id).length;
              const prog = progress[t.id];
              return (
                <div
                  key={t.id}
                  onClick={() => onOpenTopic(t)}
                  className="p-3 border-2 border-neo-border bg-[var(--neo-surface)] hover:bg-[var(--neo-surface-muted)] cursor-pointer flex justify-between items-center transition-colors"
                >
                  <div className="flex flex-col gap-0.5 min-w-0">
                    <span className="font-bold text-sm text-neo-blue truncate">{t.title}</span>
                    <span className="text-[10px] text-neo-text-muted truncate">
                      {(t.subtopics || []).length > 0 && `${t.subtopics.length} subtopics · `}
                      {Array.isArray(t.body) ? t.body[0]?.slice(0, 90) : String(t.body || '').slice(0, 90)}…
                    </span>
                  </div>
                  <div className="flex items-center gap-2 shrink-0 ml-3">
                    {topicAnns > 0 && <span className="neo-tag bg-neo-yellow text-neo-text text-[8px]">📝 {topicAnns}</span>}
                    {prog && <span className={`text-[9px] font-mono px-1.5 py-0.5 border font-bold ${prog === 'mastered' ? 'bg-neo-mint' : 'bg-neo-yellow'}`}>{prog}</span>}
                    <ChevronRight size={14} className="text-neo-text-muted" />
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {/* NOTES */}
        {tab === 'notes' && (
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <span className="neo-label-sm text-neo-text">Your notes</span>
              <div className="flex gap-2">
                <button onClick={saveNotes} className="neo-btn bg-neo-mint text-neo-text py-1 px-2 text-[10px] flex items-center gap-1">
                  {savingFlash ? <><Check size={11} /> Saved</> : <><Save size={11} /> Save</>}
                </button>
                <AIButton onClick={runSummarizeNotes} loading={loading === 'notes'}>Summarize</AIButton>
              </div>
            </div>
            <textarea
              value={notes}
              onChange={(e) => setNotesState(e.target.value)}
              placeholder={`Write markdown notes for ${domain.title}…`}
              className="neo-input w-full min-h-[200px] text-sm font-mono"
            />
            {noteSummary && (
              <div className="p-3 neo-border bg-neo-surface-muted text-xs">
                <div className="neo-label-sm text-neo-blue mb-1">AI summary</div>
                <MarkdownRenderer content={noteSummary} />
              </div>
            )}
          </div>
        )}

        {/* ROADMAP */}
        {tab === 'roadmap' && (
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <span className="neo-label-sm text-neo-text">Learning roadmap</span>
              <AIButton onClick={runRoadmap} loading={loading === 'roadmap'}>Generate roadmap</AIButton>
            </div>
            {!roadmap ? (
              <EmptyState icon={Map} title="No roadmap yet" hint="Generate an ordered path through this domain's topics with AI." />
            ) : roadmap.milestones ? (
              <ol className="flex flex-col gap-2">
                {roadmap.milestones.map((m) => (
                  <li key={m.step} className="p-3 neo-border bg-neo-surface flex items-start gap-3">
                    <span className="w-7 h-7 neo-border bg-neo-yellow text-neo-text flex items-center justify-center font-bold shrink-0">{m.step}</span>
                    <div>
                      <div className="font-bold text-sm text-neo-text">{m.title} <span className="neo-tag ml-1 text-[9px]">{m.level}</span></div>
                      <div className="text-[11px] text-neo-text-muted">{m.goal}</div>
                    </div>
                  </li>
                ))}
              </ol>
            ) : (
              <div className="p-3 neo-border bg-neo-surface-muted text-xs"><MarkdownRenderer content={roadmap.text} /></div>
            )}
          </div>
        )}

        {/* PAPERS */}
        {tab === 'papers' && (
          <div className="flex flex-col gap-3">
            <div className="p-3 neo-border bg-neo-surface-muted flex flex-col gap-2">
              <span className="neo-label-sm text-neo-text">Summarize a paper</span>
              <input value={paperTitle} onChange={(e) => setPaperTitle(e.target.value)} placeholder="Paper title" className="neo-input text-xs" />
              <textarea value={paperAbstract} onChange={(e) => setPaperAbstract(e.target.value)} placeholder="Paste the abstract or notes…" className="neo-input text-xs min-h-[70px]" />
              <AIButton onClick={runSummarizePaper} loading={loading === 'paper'} className="self-start">Summarize paper</AIButton>
            </div>
            {papers.length === 0 ? (
              <EmptyState icon={FileText} title="No papers yet" hint="Paste a title + abstract above and let AI summarize it into a card." />
            ) : (
              papers.map((p) => (
                <div key={p.id} className="p-3 neo-border bg-neo-surface flex flex-col gap-1">
                  <div className="flex justify-between items-start gap-2">
                    <span className="font-bold text-sm text-neo-text">{p.title}</span>
                    <button onClick={() => setPapers(removePaper(domain.id, p.id))} className="text-neo-text-muted hover:text-neo-red shrink-0"><Trash2 size={13} /></button>
                  </div>
                  <div className="text-xs text-neo-text-muted"><MarkdownRenderer content={p.summary} /></div>
                </div>
              ))
            )}
          </div>
        )}

        {/* PROJECTS */}
        {tab === 'projects' && (
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <span className="neo-label-sm text-neo-text">Project ideas</span>
              <AIButton onClick={runRecommend} loading={loading === 'projects'}>Recommend projects</AIButton>
            </div>
            {recommendText && (
              <div className="p-3 neo-border bg-neo-surface-muted text-xs"><MarkdownRenderer content={recommendText} /></div>
            )}
            {projects.length === 0 ? (
              <EmptyState icon={Hammer} title="No projects yet" hint="Let AI recommend hands-on projects. Accepted projects auto-add to your Repository." />
            ) : (
              projects.map((p) => (
                <div key={p.id} className="p-3 neo-border bg-neo-surface flex justify-between items-start gap-3">
                  <div className="min-w-0">
                    <div className="font-bold text-sm text-neo-text">{p.title} <span className="neo-tag ml-1 text-[9px]">{p.difficulty}</span></div>
                    <div className="text-[11px] text-neo-text-muted">{p.pitch}</div>
                  </div>
                  {p.status === 'accepted' ? (
                    <span className="neo-tag bg-neo-mint text-neo-text shrink-0"><Check size={10} /> In repo</span>
                  ) : (
                    <button onClick={() => accept(p)} className="neo-btn bg-neo-yellow text-neo-text py-1 px-2 text-[10px] flex items-center gap-1 shrink-0">
                      <Plus size={11} /> Accept → repo
                    </button>
                  )}
                </div>
              ))
            )}
          </div>
        )}
      </div>
    </div>
  );
}
