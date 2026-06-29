-- Control Plane Tables

CREATE TABLE IF NOT EXISTS users (
  id          TEXT PRIMARY KEY,
  email       TEXT NOT NULL UNIQUE,
  name        TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS memberships (
  id          TEXT PRIMARY KEY,
  user_id     TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  role        TEXT NOT NULL DEFAULT 'member', -- owner|admin|member
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL,
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id),
  UNIQUE(user_id, workspace_id)
);

CREATE TABLE IF NOT EXISTS connections (
  id            TEXT PRIMARY KEY,
  workspace_id  TEXT NOT NULL,
  provider      TEXT NOT NULL,            -- 'google'|'notion'|'slack'|'x'|'instagram'|'reddit'|'figma'|'kite'|…
  account_handle TEXT,                    -- which account (multi-account)
  nango_connection_id TEXT,               -- handle into Nango's vault (preferred path)
  secret_enc    TEXT,                     -- envelope-encrypted blob for non-Nango providers only
  scopes        TEXT,
  expires_at    INTEGER,
  status        TEXT DEFAULT 'active',    -- active|expired|revoked
  created_at    INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);

CREATE TABLE IF NOT EXISTS subscriptions (
  id            TEXT PRIMARY KEY,
  workspace_id  TEXT NOT NULL,
  plan_id       TEXT NOT NULL,
  status        TEXT NOT NULL,            -- active|past_due|canceled
  current_period_end INTEGER NOT NULL,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);
