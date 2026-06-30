#!/usr/bin/env node
// One-shot migration: knowledge-atlas's `atlasAdd()` data files -> Life OS
// generic entities (module: 'learning'), per docs/MODULES.md §2.1 and issue
// #39's "migration shim from existing atlas data" requirement.
//
// The atlas files are plain global scripts that call a global `atlasAdd`
// (see 01_Inbox/knowledge-atlas/data/_init.js for the exact merge-by-id
// contract). This script reproduces that contract in a vm sandbox, loads
// every data/*.js file in the same order index.html does, then walks the
// merged domain/topic/subtopic/resource tree and creates one entity per
// node via the running lifeos-api, preserving the original atlas id in
// attrs.atlas_id so connections can be resolved to real entity ids.
//
// Usage: node scripts/migrate-knowledge-atlas.mjs [--dry-run]
//   API_BASE / WORKSPACE_ID env vars override the defaults below.

import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';

const ATLAS_DIR = path.resolve('/Users/chayanaggarwal/Desktop/SecondBrain/01_Inbox/knowledge-atlas/data');
const API_BASE = process.env.API_BASE || 'http://127.0.0.1:8080';
const WORKSPACE_ID = process.env.WORKSPACE_ID || 'default-personal-workspace';
const DRY_RUN = process.argv.includes('--dry-run');

function loadAtlas() {
  const sandbox = { ATLAS: [] };
  sandbox.window = sandbox;
  sandbox.atlasAdd = function atlasAdd(domain) {
    const existing = sandbox.ATLAS.find((d) => d.id === domain.id);
    if (!existing) {
      sandbox.ATLAS.push(domain);
      return domain;
    }
    existing.topics = existing.topics || [];
    for (const t of domain.topics || []) {
      const prev = t.id && existing.topics.find((x) => x.id === t.id);
      if (prev) {
        for (const k of Object.keys(t)) if (t[k] != null) prev[k] = t[k];
      } else {
        existing.topics.push(t);
      }
    }
    for (const k of ['num', 'title', 'icon', 'color', 'tagline', 'overview']) {
      if (existing[k] == null && domain[k] != null) existing[k] = domain[k];
    }
    return existing;
  };
  vm.createContext(sandbox);

  // index.html's script order matters (later files enrich earlier domains by
  // id) - _init.js first, then every other file in directory order, which
  // matches the numeric/alphabetic prefixes index.html lists them in.
  const files = readdirSync(ATLAS_DIR).filter((f) => f.endsWith('.js')).sort();
  const ordered = ['_init.js', ...files.filter((f) => f !== '_init.js')];
  for (const file of ordered) {
    const src = readFileSync(path.join(ATLAS_DIR, file), 'utf8');
    vm.runInContext(src, sandbox, { filename: file });
  }
  return sandbox.ATLAS;
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

async function migrate() {
  const atlas = loadAtlas();
  console.log(`Loaded ${atlas.length} domains from knowledge-atlas.`);

  let counts = { domain: 0, topic: 0, subtopic: 0, resource: 0, edge: 0 };
  // atlas_id -> created entity id, so connections can resolve to real edges.
  const topicEntityIdByAtlasId = new Map();
  const pendingConnections = []; // [{fromEntityId, toAtlasId, note}]

  for (const domain of atlas) {
    const { data: domainEntity } = await post('/api/entity', {
      module: 'learning',
      type: 'domain',
      title: domain.title,
      attrs: {
        atlas_id: domain.id,
        icon: domain.icon,
        color: domain.color,
        tagline: domain.tagline,
        overview: domain.overview || [],
        num: domain.num,
      },
    });
    counts.domain++;

    for (const topic of domain.topics || []) {
      const { data: topicEntity } = await post('/api/entity', {
        module: 'learning',
        type: 'topic',
        parent_id: domainEntity.id,
        title: topic.title,
        attrs: {
          atlas_id: topic.id,
          level: topic.level,
          body: topic.body || [],
          mastery: null,
          last_review: null,
          next_due: null,
        },
      });
      counts.topic++;
      if (topic.id) topicEntityIdByAtlasId.set(topic.id, topicEntity.id);

      for (const sub of topic.subtopics || []) {
        await post('/api/entity', {
          module: 'learning',
          type: 'subtopic',
          parent_id: topicEntity.id,
          title: sub.title,
          attrs: { body: sub.body || [] },
        });
        counts.subtopic++;
        for (const r of sub.resources || []) {
          await post('/api/entity', {
            module: 'learning',
            type: 'resource',
            parent_id: topicEntity.id,
            title: r.label,
            attrs: { url: r.url, kind: r.type, note: r.note },
          });
          counts.resource++;
        }
      }
      for (const r of topic.resources || []) {
        await post('/api/entity', {
          module: 'learning',
          type: 'resource',
          parent_id: topicEntity.id,
          title: r.label,
          attrs: { url: r.url, kind: r.type, note: r.note },
        });
        counts.resource++;
      }
      for (const c of topic.connections || []) {
        pendingConnections.push({ fromEntityId: topicEntity.id, toAtlasId: c.to, note: c.note });
      }
    }
    console.log(`  domain '${domain.id}': ${(domain.topics || []).length} topics`);
  }

  // Cross-domain topic connections become edges once every topic id is
  // known (a connection can point forward to a domain/topic not yet seen).
  for (const c of pendingConnections) {
    const toEntityId = topicEntityIdByAtlasId.get(c.toAtlasId);
    if (!toEntityId) continue; // points at a domain id or unresolvable target - skip rather than guess
    await post('/api/edge', { src_id: c.fromEntityId, dst_id: toEntityId, rel: 'connection', attrs: { note: c.note } });
    counts.edge++;
  }

  console.log('Migration complete:', counts);
}

migrate().catch((e) => {
  console.error('Migration failed:', e);
  process.exit(1);
});
