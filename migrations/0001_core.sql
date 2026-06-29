-- Core Data Plane Tables

CREATE TABLE IF NOT EXISTS workspaces (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  plan        TEXT DEFAULT 'free',
  limits      TEXT NOT NULL DEFAULT '{}',
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS entities (
  id          TEXT PRIMARY KEY,           -- ulid
  workspace_id TEXT NOT NULL,             -- tenant scope (always present)
  module      TEXT NOT NULL,              -- 'learning' | 'tasks' | 'email' | …
  type        TEXT NOT NULL,              -- 'topic' | 'task' | 'trade' | 'email' | 'asset' | …
  parent_id   TEXT,                       -- hierarchy (nullable)
  title       TEXT,                       -- denormalized display title
  status      TEXT,                       -- lifecycle state per module manifest
  tier        TEXT,                       -- optional ranking/priority bucket
  attrs       TEXT NOT NULL DEFAULT '{}', -- JSON escape hatch (the per-domain fields)
  source      TEXT,                       -- provenance ('telegram'|'gmail'|'agent'|…)
  blob_ref    TEXT,                       -- content hash into lifeos-vcs (for file-bearing entities)
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);

CREATE INDEX IF NOT EXISTS ix_entities_ws_module_type ON entities(workspace_id, module, type);
CREATE INDEX IF NOT EXISTS ix_entities_parent ON entities(parent_id);

CREATE TABLE IF NOT EXISTS edges (
  id          TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  src_id      TEXT NOT NULL,              -- entity
  dst_id      TEXT,                       -- entity (nullable for external refs)
  dst_ref     TEXT,                       -- external target (URL, notion_page_id, …)
  rel         TEXT NOT NULL,              -- 'connection'|'depends_on'|'blocks'|'derived_from'
                                          -- 'owns'|'publishes_to'|'uses_asset'|'thesis'|'same_as'|…
  state       TEXT DEFAULT 'accepted',    -- 'pending'|'accepted'
  created_by  TEXT,                       -- 'agent'|'user'|module id
  created_at  INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);

CREATE INDEX IF NOT EXISTS ix_edges_src ON edges(workspace_id, src_id);
CREATE INDEX IF NOT EXISTS ix_edges_dst ON edges(workspace_id, dst_id);

CREATE TABLE IF NOT EXISTS events (
  id          TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  ts          INTEGER NOT NULL,
  type        TEXT NOT NULL,              -- domain: 'study.review'|'task.completed'|'trade.closed'
                                          -- |'post.published'|'version.created'|'module.installed'…
  entity_id   TEXT,                       -- subject (nullable)
  actor       TEXT,                       -- 'user'|'bot'|'harness'|module
  attrs       TEXT DEFAULT '{}',          -- payload
  -- harness run-log columns (events doubles as the run log):
  run_id      TEXT,
  tier        TEXT,
  model       TEXT,
  tokens_in   INTEGER,
  tokens_out  INTEGER,
  cost        REAL,
  latency_ms  INTEGER,
  error       TEXT,
  outcome     TEXT,
  eval_score  REAL,
  gated       INTEGER DEFAULT 0,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);

CREATE INDEX IF NOT EXISTS ix_events_ws_ts ON events(workspace_id, ts);
CREATE INDEX IF NOT EXISTS ix_events_type ON events(workspace_id, type);

CREATE TABLE IF NOT EXISTS jobs (
  id          TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  kind        TEXT NOT NULL,              -- 'ingest'|'pipeline'|'module_build'|'eval'|…
  payload     TEXT NOT NULL DEFAULT '{}',
  status      TEXT NOT NULL DEFAULT 'queued', -- queued|running|done|failed
  priority    INTEGER DEFAULT 0,
  run_after   INTEGER,                    -- delayed jobs
  claimed_by  TEXT,
  claimed_at  INTEGER,                    -- lease (for crash recovery)
  attempts    INTEGER DEFAULT 0,
  created_at  INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);

CREATE INDEX IF NOT EXISTS ix_jobs_claim ON jobs(status, priority DESC, created_at);

CREATE TABLE IF NOT EXISTS module_requests (
  id           TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  prompt       TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'queued', -- queued|building|installed|failed
  error        TEXT,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);
