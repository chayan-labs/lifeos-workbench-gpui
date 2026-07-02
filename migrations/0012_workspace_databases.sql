-- Database-per-workspace provisioning record (issue #104). One row per
-- workspace that has had a dedicated Turso database provisioned; the token
-- is envelope-encrypted under that workspace's own key (0011), never stored
-- or returned in plaintext.
CREATE TABLE IF NOT EXISTS workspace_databases (
  workspace_id    TEXT PRIMARY KEY,
  turso_db_name   TEXT NOT NULL,
  turso_db_url    TEXT NOT NULL,
  turso_token_enc TEXT NOT NULL,
  created_at      INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);
