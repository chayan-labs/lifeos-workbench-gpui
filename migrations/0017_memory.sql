-- 0017_memory.sql - cognitive memory read models (issues #111/#112/#115/#117/
-- #118, docs/AI-MEMORY.md §3). EVERY table here is a DERIVED, rebuildable
-- projection of the append-only `events` log: it can be deleted per workspace
-- and recomputed identically by the projector (lifeos-memory). Rows live in
-- the canonical DB (metadata, cheap, syncable); the FTS index over them lives
-- in the never-synced derived DB (0018).

-- Semantic/episodic memory items. One row ~= one remembered thing, distilled
-- from one or more events. Bi-temporal validity mirrors memory_edges so a
-- superseded FACT (not just a relation) is never returned as current truth
-- while point-in-time queries still recover it (docs/AI-MEMORY.md §7).
CREATE TABLE IF NOT EXISTS memory_nodes (
  id                     TEXT PRIMARY KEY,             -- deterministic: mn_<blake3(ws|event_id)>
  workspace_id           TEXT NOT NULL,
  kind                   TEXT NOT NULL DEFAULT 'episodic', -- 'episodic'|'semantic'
  content                TEXT NOT NULL,                -- redacted, flattened, searchable text
  importance             REAL NOT NULL DEFAULT 0.3,    -- salience, scored at write time (§4)
  access_count           INTEGER NOT NULL DEFAULT 0,   -- ACT-R base-level activation input
  last_accessed          INTEGER,                      -- unix secs
  confidence             REAL NOT NULL DEFAULT 1.0,    -- consolidation confidence (§5)
  source_event_ids       TEXT NOT NULL DEFAULT '[]',   -- JSON array - provenance is mandatory
  embedding_ref          TEXT,                         -- pointer into derived vec index (memvec)
  tiered_ref             TEXT,                         -- blob hash when cold-tiered out (§7)
  ts                     INTEGER NOT NULL,             -- when the memory became valid
  t_invalid              INTEGER,                      -- NULL = current truth
  superseded_by_event_id TEXT,                         -- the event that invalidated it
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);

CREATE INDEX IF NOT EXISTS ix_memory_nodes_ws_ts ON memory_nodes(workspace_id, ts);
CREATE INDEX IF NOT EXISTS ix_memory_nodes_ws_valid ON memory_nodes(workspace_id, t_invalid);

-- Bi-temporal relations between memories and entities (Zep/Graphiti model).
-- Spreading activation (petgraph, issue #114) walks rows where t_invalid IS NULL.
CREATE TABLE IF NOT EXISTS memory_edges (
  id                     TEXT PRIMARY KEY,             -- deterministic: me_<blake3(...)>
  workspace_id           TEXT NOT NULL,
  from_id                TEXT NOT NULL,                -- memory_nodes.id
  to_id                  TEXT NOT NULL,                -- memory_nodes.id | entities.id
  rel_type               TEXT NOT NULL,                -- 'about'|'caused_by'|'part_of_episode'|…
  t_valid                INTEGER NOT NULL,
  t_invalid              INTEGER,                      -- NULL = currently true
  superseded_by_event_id TEXT,
  source_event_id        TEXT NOT NULL,                -- provenance is mandatory
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);

CREATE INDEX IF NOT EXISTS ix_memory_edges_ws_from ON memory_edges(workspace_id, from_id);
CREATE INDEX IF NOT EXISTS ix_memory_edges_ws_to ON memory_edges(workspace_id, to_id);

-- Multi-resolution summaries written by consolidation "sleep" jobs (§5).
-- Materialized from memory.summary.created events, so a rebuild replays them
-- byte-identically without re-calling any model.
CREATE TABLE IF NOT EXISTS memory_summaries (
  id               TEXT PRIMARY KEY,                   -- deterministic: ms_<blake3(ws|event_id)>
  workspace_id     TEXT NOT NULL,
  level            TEXT NOT NULL,                      -- 'episode'|'day'|'week'|'profile'
  content          TEXT NOT NULL,
  confidence       REAL NOT NULL DEFAULT 0.5,
  source_event_ids TEXT NOT NULL DEFAULT '[]',
  ts               INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);

CREATE INDEX IF NOT EXISTS ix_memory_summaries_ws ON memory_summaries(workspace_id, level, ts);

-- Procedural memory (issue #118, §8): learned behavioral rules that feed the
-- system prompt. Materialized from memory.rule.added / memory.rule.retired
-- events; never edited in place.
CREATE TABLE IF NOT EXISTS memory_rules (
  id               TEXT PRIMARY KEY,                   -- deterministic: mr_<blake3(ws|event_id)>
  workspace_id     TEXT NOT NULL,
  rule             TEXT NOT NULL,
  confidence       REAL NOT NULL DEFAULT 0.5,
  status           TEXT NOT NULL DEFAULT 'active',     -- 'active'|'retired'
  source_event_ids TEXT NOT NULL DEFAULT '[]',
  created_ts       INTEGER NOT NULL,
  retired_ts       INTEGER,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);

CREATE INDEX IF NOT EXISTS ix_memory_rules_ws ON memory_rules(workspace_id, status);

-- Content-addressed LLM replay cache (issue #112, §2): BLAKE3(request) ->
-- response. This is what makes "deterministic rebuild" true despite LLM
-- non-determinism, so it lives in the CANONICAL db (blowing away the derived
-- store must not lose it).
CREATE TABLE IF NOT EXISTS llm_replay_cache (
  request_hash TEXT PRIMARY KEY,                       -- blake3(task || prompt)
  task         TEXT NOT NULL,                          -- 'summarize'|'reflect'|… (debuggability)
  response     TEXT NOT NULL,
  created_at   INTEGER NOT NULL
);

-- Projection/consolidation cursors: how far into `events` each pass has read.
-- Derived bookkeeping (a rebuild resets it), kept canonical so drain and the
-- API share one position.
CREATE TABLE IF NOT EXISTS memory_cursors (
  workspace_id         TEXT PRIMARY KEY,
  projected_ts         INTEGER NOT NULL DEFAULT 0,
  projected_id         TEXT NOT NULL DEFAULT '',
  consolidated_ts      INTEGER NOT NULL DEFAULT 0,
  consolidated_id      TEXT NOT NULL DEFAULT '',
  updated_at           INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);
