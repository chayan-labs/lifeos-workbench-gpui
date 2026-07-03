// localStorage-backed overlay for the Knowledge Atlas. The base domains/topics
// ship in atlas_data.json (read-only); everything the user adds on top - custom
// domains, per-domain notes, paper summaries, accepted projects - lives here so
// any domain can grow into a full learning/notes/roadmap workspace. Accepted
// projects are mirrored into the Repository/Modules store so the "accept ->
// auto-add to repo" loop works without a backend.

const K = {
  domains: 'KA_CUSTOM_DOMAINS_V1', // user-scaffolded domains (array)
  notes: 'KA_DOMAIN_NOTES_V1',     // { [domainId]: markdownString }
  papers: 'KA_PAPERS_V1',          // { [domainId]: [{id,title,summary,url,addedAt}] }
  projects: 'KA_PROJECTS_V1',      // { [domainId]: [{id,title,pitch,difficulty,status}] }
  repo: 'KA_REPO_PROJECTS_V1',     // mirror consumed by the Repository page
};

const read = (key, fallback) => {
  try {
    const raw = localStorage.getItem(key);
    return raw ? JSON.parse(raw) : fallback;
  } catch {
    return fallback;
  }
};
const write = (key, value) => localStorage.setItem(key, JSON.stringify(value));

// ---- Custom domains -------------------------------------------------------
export const getCustomDomains = () => read(K.domains, []);

export const addCustomDomain = (domain) => {
  const existing = getCustomDomains();
  const next = [...existing.filter((d) => d.id !== domain.id), domain];
  write(K.domains, next);
  return next;
};

export const removeCustomDomain = (id) => {
  const next = getCustomDomains().filter((d) => d.id !== id);
  write(K.domains, next);
  return next;
};

// ---- Notes ----------------------------------------------------------------
export const getNotes = (domainId) => read(K.notes, {})[domainId] || '';

export const setNotes = (domainId, markdown) => {
  const all = read(K.notes, {});
  all[domainId] = markdown;
  write(K.notes, all);
};

// ---- Papers ---------------------------------------------------------------
export const getPapers = (domainId) => read(K.papers, {})[domainId] || [];

export const addPaper = (domainId, paper) => {
  const all = read(K.papers, {});
  const list = all[domainId] || [];
  all[domainId] = [{ ...paper, id: paper.id || `p_${Date.now().toString(36)}`, addedAt: new Date().toISOString() }, ...list];
  write(K.papers, all);
  return all[domainId];
};

export const removePaper = (domainId, paperId) => {
  const all = read(K.papers, {});
  all[domainId] = (all[domainId] || []).filter((p) => p.id !== paperId);
  write(K.papers, all);
  return all[domainId];
};

// ---- Projects (with repo mirror) -----------------------------------------
export const getProjects = (domainId) => read(K.projects, {})[domainId] || [];

export const addProject = (domainId, project) => {
  const all = read(K.projects, {});
  const list = all[domainId] || [];
  const entry = { ...project, id: project.id || `pj_${Date.now().toString(36)}`, status: project.status || 'suggested' };
  all[domainId] = [entry, ...list.filter((p) => p.id !== entry.id)];
  write(K.projects, all);
  return all[domainId];
};

// Accepting a project moves it to 'accepted' and pushes it into the repo mirror,
// which the Repository page reads as an auto-created project folder.
export const acceptProject = (domainId, domainTitle, project) => {
  const all = read(K.projects, {});
  all[domainId] = (all[domainId] || []).map((p) =>
    p.id === project.id ? { ...p, status: 'accepted' } : p
  );
  write(K.projects, all);

  const repo = read(K.repo, []);
  if (!repo.some((r) => r.id === project.id)) {
    repo.unshift({
      id: project.id,
      name: project.title,
      domain: domainTitle,
      domainId,
      pitch: project.pitch,
      difficulty: project.difficulty,
      acceptedAt: new Date().toISOString(),
    });
    write(K.repo, repo);
  }
  return all[domainId];
};

export const getRepoProjects = () => read(K.repo, []);
