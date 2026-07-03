#!/usr/bin/env node
// Coding/Projects module seeder (issue #41, docs/MODULES.md §2.3): scans the
// git repos under 04_Projects and upserts one `repo` entity (module:
// 'coding') per repo, with real attrs read from git itself - no fake CI
// status, no guessed gaps. GitHub-sourced state (issues/PRs/real CI runs) is
// explicitly deferred to the Nango integration (Phase 3, docs/INTEGRATIONS.md)
// per this issue's scope; for now `ci_state` only reflects whether a
// `.github/workflows/` directory exists locally, which is honest and free.
//
// Usage: node scripts/scan-projects.mjs [--dry-run]
//   PROJECTS_DIR / API_BASE / WORKSPACE_ID env vars override the defaults.

import { execSync } from 'node:child_process';
import { existsSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';

const PROJECTS_DIR = process.env.PROJECTS_DIR || path.resolve('/Users/chayanaggarwal/Desktop/SecondBrain/04_Projects');
const API_BASE = process.env.API_BASE || 'http://127.0.0.1:8080';
const WORKSPACE_ID = process.env.WORKSPACE_ID || 'default-personal-workspace';
const DRY_RUN = process.argv.includes('--dry-run');

function git(dir, args) {
  try {
    return execSync(`git ${args}`, { cwd: dir, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  } catch {
    return null;
  }
}

function scanRepo(dir) {
  const remote = git(dir, 'remote get-url origin');
  const defaultBranch = git(dir, 'rev-parse --abbrev-ref HEAD');
  const lastCommitLine = git(dir, 'log -1 --format=%H|%s|%ct');
  const [lastCommitHash, lastCommitMessage, lastCommitTs] = lastCommitLine ? lastCommitLine.split('|') : [null, null, null];
  const hasWorkflows = existsSync(path.join(dir, '.github', 'workflows'));
  const dirty = git(dir, 'status --porcelain');

  return {
    path: dir,
    remote,
    default_branch: defaultBranch,
    last_commit: lastCommitHash ? { hash: lastCommitHash, message: lastCommitMessage, ts: Number(lastCommitTs) } : null,
    // Honest, local-only signal until GitHub Actions data flows in via Nango
    // (Phase 3) - we are not faking a pass/fail we cannot observe.
    ci_state: hasWorkflows ? 'configured (status unknown - GitHub integration pending)' : 'none',
    dirty: Boolean(dirty),
  };
}

function findRepos(root) {
  const repos = [];
  for (const entry of readdirSync(root)) {
    const full = path.join(root, entry);
    if (!statSync(full).isDirectory()) continue;
    if (existsSync(path.join(full, '.git'))) repos.push(full);
  }
  return repos;
}

async function post(pathSuffix, body) {
  if (DRY_RUN) return { ok: true, data: { id: `dry_${Math.random().toString(36).slice(2)}` } };
  const res = await fetch(`${API_BASE}${pathSuffix}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'X-Workspace-Id': WORKSPACE_ID },
    body: JSON.stringify(body),
  });
  const data = await res.json().catch(() => null);
  if (!res.ok) throw new Error(`POST ${pathSuffix} -> ${res.status}: ${JSON.stringify(data)}`);
  return { ok: true, data };
}

async function findExistingRepoEntity(repoPath) {
  if (DRY_RUN) return null;
  const res = await fetch(`${API_BASE}/api/entity?module=coding&type=repo&limit=200`, {
    headers: { 'X-Workspace-Id': WORKSPACE_ID },
  });
  const data = await res.json().catch(() => []);
  return (data || []).find((e) => e.attrs?.path === repoPath) || null;
}

async function seed() {
  const repoDirs = findRepos(PROJECTS_DIR);
  console.log(`Found ${repoDirs.length} git repos under ${PROJECTS_DIR}.`);

  let created = 0;
  let updated = 0;
  for (const dir of repoDirs) {
    const name = path.basename(dir);
    const info = scanRepo(dir);
    const existing = await findExistingRepoEntity(dir);

    if (existing) {
      await fetch(`${API_BASE}/api/entity/${existing.id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json', 'X-Workspace-Id': WORKSPACE_ID },
        body: JSON.stringify({ attrs: { ...existing.attrs, ...info } }),
      });
      updated++;
      console.log(`  updated repo '${name}'`);
      continue;
    }

    await post('/api/entity', {
      module: 'coding',
      type: 'repo',
      title: name,
      status: info.dirty ? 'dirty' : 'clean',
      attrs: info,
    });
    created++;
    console.log(`  created repo '${name}' (ci_state=${info.ci_state})`);
  }

  console.log(`Done: ${created} created, ${updated} updated.`);
}

seed().catch((e) => {
  console.error('Project scan failed:', e);
  process.exit(1);
});
