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

-- Many accounts per provider per workspace -> index the lookup, no UNIQUE.
CREATE INDEX IF NOT EXISTS ix_connections_ws_provider ON connections(workspace_id, provider);

-- Billing seam (stub now; gates module/quota access in SaaS). plans is the catalog,
-- subscriptions joins a workspace to a plan. Personal use rides the seeded 'free' plan.
CREATE TABLE IF NOT EXISTS plans (
  id          TEXT PRIMARY KEY,           -- 'free'|'pro'|'team'|…
  name        TEXT NOT NULL,
  price_cents INTEGER NOT NULL DEFAULT 0, -- monthly price in minor units
  currency    TEXT NOT NULL DEFAULT 'usd',
  limits      TEXT NOT NULL DEFAULT '{}', -- JSON quota/feature gates (modules, storage, jobs/day, …)
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
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
