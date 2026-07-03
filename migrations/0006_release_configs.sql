-- Release-loop candidate configs (issue #98, docs/HARNESS-LOOP.md §4).
-- A candidate is a versioned, workspace-scoped JSON payload (currently only
-- kind='route_prior' - a learned reranking bias for route_core.py); it
-- moves draft -> shadow -> promoted|rejected. The *active* pointer per
-- kind is a vcs_refs row (kind='config_active', name=<configs.kind>,
-- snapshot_ref=<configs.id>) - same named-pointer/atomic-flip shape #84
-- already built for branches/tags, so no second pointer table is needed.

CREATE TABLE IF NOT EXISTS configs (
  id             TEXT PRIMARY KEY,
  workspace_id   TEXT NOT NULL,
  kind           TEXT NOT NULL,
  payload        TEXT NOT NULL,
  status         TEXT NOT NULL, -- 'draft' | 'shadow' | 'promoted' | 'rejected'
  shadow_summary TEXT,
  created_at     INTEGER NOT NULL,
  promoted_at    INTEGER
);

CREATE INDEX IF NOT EXISTS idx_configs_ws_kind_status
  ON configs (workspace_id, kind, status);
