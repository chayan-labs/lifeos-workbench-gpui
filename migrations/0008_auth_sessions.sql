-- Session/refresh-token rotation (issue #100, docs/SECURITY.md §5). Access
-- tokens (`issue_token`, auth.rs) are unchanged - still a stateless HS256
-- JWT verified by the existing `resolve_workspace` used everywhere;
-- sessions are the new stateful layer behind them, letting a refresh
-- token be rotated and revoked without touching the 23 routes that
-- already call `resolve_workspace`.

CREATE TABLE IF NOT EXISTS sessions (
  id                  TEXT PRIMARY KEY,
  user_id             TEXT NOT NULL,
  workspace_id        TEXT NOT NULL,
  refresh_token_hash  TEXT NOT NULL,
  created_at          INTEGER NOT NULL,
  expires_at          INTEGER NOT NULL,
  revoked_at          INTEGER
);

CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions (user_id);
