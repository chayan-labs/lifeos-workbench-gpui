// AI affordance layer. Every "do it with AI" surface in the app routes through
// here so behavior is consistent and there is always a graceful local fallback
// when the /api/llm backend lane is not yet wired. The agent is meant to be
// available at every layer (scaffold a domain, generate a roadmap, summarize
// notes/papers, recommend projects) - these are the contracts for that.

import { apiCall } from './api';

export const SELECTED_AGENT_KEY = 'life_os_selected_agent';
export const SELECTED_MODEL_KEY = 'life_os_selected_model';
// Fired by AgentPicker whenever the agent or model changes, so every mounted
// picker (console, harness, ...) stays in sync without prop drilling.
export const AGENT_CHANGED_EVENT = 'lifeos:agent-changed';

// The agent/model picked in any AgentPicker (or undefined for the backend's
// default). Exported so every direct apiCall('POST', '/api/llm', ...) call
// site applies the user's choice, not just callers that go through
// complete() below.
export function selectedAgent() {
  return localStorage.getItem(SELECTED_AGENT_KEY) || undefined;
}

export function selectedModel() {
  return localStorage.getItem(SELECTED_MODEL_KEY) || undefined;
}

// The standard body fields every /api/llm call should spread in, so agent
// and model switching applies everywhere AI is used.
export function llmSelection() {
  return { agent: selectedAgent(), model: selectedModel() };
}

async function complete(system, prompt) {
  const { ok, data } = await apiCall('POST', '/api/llm', { system, prompt, ...llmSelection() });
  if (ok && data && (data.text || typeof data === 'string')) {
    return data.text || data;
  }
  return null; // caller supplies a deterministic mock fallback
}

const slug = (s) =>
  String(s || '').toLowerCase().trim().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');

// Scaffold a brand-new domain workspace from a name/intent. Any domain becomes
// addable: AI proposes an overview, a topic skeleton and a starter roadmap.
export async function scaffoldDomain(name, intent) {
  const sys = 'You scaffold a new learning domain. Return an overview, 4-6 topics, and a 4-step roadmap.';
  const text = await complete(sys, `Domain: ${name}\nIntent: ${intent || 'general mastery'}`);
  const id = slug(name);
  if (text) {
    return { id, title: name, overview: text, ai: true };
  }
  // Mock fallback: a believable skeleton so the flow works offline.
  return {
    id,
    title: name,
    icon: '✦',
    overview: `**${name}** - a new self-authored domain.\n\nThis workspace was scaffolded locally (the \`/api/llm\` lane is not connected yet). Add topics, notes, papers and projects; the AI will enrich each as soon as the backend is live.`,
    topics: [
      { id: `${id}-foundations`, title: `${name}: Foundations`, level: 'Beginner', body: [`Core concepts and vocabulary of ${name}.`], subtopics: [], resources: [], connections: [] },
      { id: `${id}-core`, title: `${name}: Core Models`, level: 'Intermediate', body: [`The central models and how they fit together.`], subtopics: [], resources: [], connections: [] },
      { id: `${id}-applied`, title: `${name}: Applied Practice`, level: 'Advanced', body: [`Putting ${name} to work on real problems.`], subtopics: [], resources: [], connections: [] },
    ],
    ai: false,
  };
}

// Generate an ordered roadmap (milestones) for a domain from its topics.
export async function generateRoadmap(domain) {
  const sys = 'You are a curriculum planner. Output an ordered roadmap of 4-6 milestones with a one-line goal each.';
  const topics = (domain.topics || []).map((t) => t.title).join(', ');
  const text = await complete(sys, `Domain: ${domain.title}\nTopics: ${topics}`);
  if (text) return { text, ai: true };
  // Mock fallback: derive milestones directly from topics + levels.
  const order = { Beginner: 0, Intermediate: 1, Advanced: 2 };
  const sorted = [...(domain.topics || [])].sort(
    (a, b) => (order[a.level] ?? 1) - (order[b.level] ?? 1)
  );
  const milestones = sorted.map((t, i) => ({
    step: i + 1,
    title: t.title,
    level: t.level || 'Intermediate',
    goal: `Reach working fluency in ${t.title}.`,
  }));
  return { milestones, ai: false };
}

// Summarize a learner's notes for a domain into key takeaways.
export async function summarizeNotes(domainTitle, notes) {
  const sys = 'Summarize these study notes into 3-5 crisp takeaways and 1 open question.';
  const text = await complete(sys, `Domain: ${domainTitle}\nNotes:\n${notes}`);
  return text || `**Takeaways (local summary)**\n\n- ${notes.split('\n').filter(Boolean).slice(0, 4).join('\n- ') || 'No notes yet.'}\n\n*Connect \`/api/llm\` for an AI-written synthesis.*`;
}

// Summarize a pasted paper (title + abstract/url) into a structured card.
export async function summarizePaper(title, abstract) {
  const sys = 'Summarize this research paper: problem, method, key result, and why it matters. Be concise.';
  const text = await complete(sys, `Title: ${title}\nAbstract/Notes: ${abstract}`);
  if (text) return text;
  return `**${title}**\n\n- **Problem:** ${(abstract || '').slice(0, 120) || 'paste an abstract to summarize'}…\n- **Method:** _pending AI summary_\n- **Result:** _pending AI summary_\n\n*Connect \`/api/llm\` to auto-summarize.*`;
}

// Recommend projects for a domain. Accepted ones get pushed into the repo module.
export async function recommendProjects(domain) {
  const sys = 'Recommend 3 hands-on projects for this domain. For each: title, one-line pitch, difficulty.';
  const topics = (domain.topics || []).map((t) => t.title).join(', ');
  const text = await complete(sys, `Domain: ${domain.title}\nTopics: ${topics}`);
  if (text) {
    // Best-effort: backend may return structured JSON; if plain text, wrap it.
    return { text, ai: true };
  }
  const base = domain.title;
  return {
    projects: [
      { id: `${slug(base)}-proj-1`, title: `${base} starter build`, pitch: `Implement the core idea of ${base} end to end.`, difficulty: 'Beginner' },
      { id: `${slug(base)}-proj-2`, title: `${base} in the wild`, pitch: `Apply ${base} to a real dataset / repo and write it up.`, difficulty: 'Intermediate' },
      { id: `${slug(base)}-proj-3`, title: `${base} from scratch`, pitch: `Rebuild a ${base} tool from first principles.`, difficulty: 'Advanced' },
    ],
    ai: false,
  };
}
