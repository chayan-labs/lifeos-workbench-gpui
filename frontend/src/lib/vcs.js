// Local VCS + time-travel for the self-evolving app (frontend showcase).
//
// Every meaningful change - by the user OR by AI - can be committed here. The
// log is append-only: restoring a past point does NOT rewrite history, it
// appends a new "restore" commit, so you can always move forward again.
//
// VCS is GATED from AI (see capabilities.js): commits are human-authored only.
// commit() refuses author='ai' by design - the agent can propose changes, but
// only the human writes them into version history.

const LOG_KEY = 'LIFEOS_VCS_LOG_V1';

// The slice of app state that VCS tracks. Each "file" is a localStorage key.
// This is the unit of both snapshot and per-file restore.
export const TRACKED_KEYS = [
  'life_os_theme',
  'life_os_sidebar_collapsed',
  'life_os_user_name',
  'life_os_workspace_name',
  'life_os_plan',
  'life_os_storage_backend',
  'life_os_harness_state',
  'KA_ANNOTATIONS_V1',
  'KA_PROGRESS_V1',
  'KA_USERCONN_V1',
  'KA_CUSTOM_DOMAINS_V1',
  'KA_DOMAIN_NOTES_V1',
  'KA_PAPERS_V1',
  'KA_PROJECTS_V1',
  'KA_REPO_PROJECTS_V1',
];

const readLog = () => {
  try {
    return JSON.parse(localStorage.getItem(LOG_KEY)) || [];
  } catch {
    return [];
  }
};
const writeLog = (log) => localStorage.setItem(LOG_KEY, JSON.stringify(log));

// Capture the current value of every tracked key.
export function snapshotNow() {
  const snap = {};
  for (const k of TRACKED_KEYS) {
    const v = localStorage.getItem(k);
    if (v !== null) snap[k] = v;
  }
  return snap;
}

const hash = (obj) => {
  // Tiny deterministic content hash for display (not cryptographic).
  const s = JSON.stringify(obj);
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return 'b3:' + (h >>> 0).toString(16).padStart(8, '0');
}

export function listCommits() {
  return readLog();
}

export function getBaseline() {
  return readLog().find((c) => c.baseline) || null;
}

// Ensure an immutable baseline commit exists representing the app as it is now.
// Called once on first run; the user's chosen "core starting state".
export function ensureBaseline() {
  const log = readLog();
  if (log.some((c) => c.baseline)) return log;
  const snapshot = snapshotNow();
  const commit = {
    id: 'c_baseline',
    message: 'Baseline - core starting state (protected)',
    author: 'system',
    baseline: true,
    parent: null,
    createdAt: new Date().toISOString(),
    hash: hash(snapshot),
    snapshot,
  };
  writeLog([commit]);
  return [commit];
}

// Append a human commit. AI is gated: author='ai' is rejected.
export function commit(message, author = 'user') {
  if (author === 'ai') {
    throw new Error('VCS is gated from AI. Commits are human-authored only.');
  }
  const log = readLog();
  const parent = log.length ? log[log.length - 1].id : null;
  const snapshot = snapshotNow();
  const c = {
    id: 'c_' + Date.now().toString(36),
    message: message?.trim() || 'Update',
    author,
    baseline: false,
    parent,
    createdAt: new Date().toISOString(),
    hash: hash(snapshot),
    snapshot,
  };
  writeLog([...log, c]);
  return c;
}

const applySnapshot = (snapshot, keys) => {
  for (const k of keys) {
    if (snapshot[k] !== undefined) {
      localStorage.setItem(k, snapshot[k]);
    } else if (TRACKED_KEYS.includes(k)) {
      localStorage.removeItem(k);
    }
  }
};

// Full-snapshot jump: restore every tracked key to a commit, then append a
// restore commit so the move is itself part of history.
export function restoreSnapshot(commitId) {
  const log = readLog();
  const target = log.find((c) => c.id === commitId);
  if (!target) throw new Error('Commit not found.');
  applySnapshot(target.snapshot, TRACKED_KEYS);
  const restore = {
    id: 'c_' + Date.now().toString(36),
    message: `Time-travel: restored "${target.message}"`,
    author: 'user',
    baseline: false,
    parent: log[log.length - 1].id,
    restoredFrom: commitId,
    createdAt: new Date().toISOString(),
    hash: target.hash,
    snapshot: target.snapshot,
  };
  writeLog([...log, restore]);
  return restore;
}

// Per-file restore: bring back a single tracked key from a commit, leaving the
// rest of the app at its current state.
export function restoreFile(commitId, key) {
  const log = readLog();
  const target = log.find((c) => c.id === commitId);
  if (!target) throw new Error('Commit not found.');
  applySnapshot(target.snapshot, [key]);
  const restore = {
    id: 'c_' + Date.now().toString(36),
    message: `Time-travel: restored file "${key}" from "${target.message}"`,
    author: 'user',
    baseline: false,
    parent: log[log.length - 1].id,
    restoredFrom: commitId,
    restoredFile: key,
    createdAt: new Date().toISOString(),
    hash: hash(snapshotNow()),
    snapshot: snapshotNow(),
  };
  writeLog([...log, restore]);
  return restore;
}

// Which tracked keys differ from the latest commit (the working-tree diff).
export function dirtyKeys() {
  const log = readLog();
  if (!log.length) return TRACKED_KEYS.filter((k) => localStorage.getItem(k) !== null);
  const last = log[log.length - 1].snapshot;
  const now = snapshotNow();
  const keys = new Set([...Object.keys(last), ...Object.keys(now)]);
  return [...keys].filter((k) => last[k] !== now[k]);
}
