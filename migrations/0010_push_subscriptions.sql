-- PWA Web Push subscriptions (issue #103). Storage only - actual VAPID
-- push delivery is a separate deferred piece (docs/PLATFORM-SYSTEMS.md).
CREATE TABLE IF NOT EXISTS push_subscriptions (
  id            TEXT PRIMARY KEY,
  workspace_id  TEXT NOT NULL,
  endpoint      TEXT NOT NULL,
  keys_json     TEXT NOT NULL,
  created_at    INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id),
  UNIQUE(workspace_id, endpoint)
);
