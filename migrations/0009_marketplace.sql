-- Module marketplace (issues #101, #102). One row per published version;
-- signature + publisher_pubkey let any installer re-verify locally without
-- trusting the registry transport.
CREATE TABLE IF NOT EXISTS module_packages (
  id                TEXT PRIMARY KEY,
  workspace_id      TEXT NOT NULL,
  module_id         TEXT NOT NULL,
  version           TEXT NOT NULL,
  manifest_json     TEXT NOT NULL,
  signature         TEXT NOT NULL,
  publisher_pubkey  TEXT NOT NULL,
  created_at        INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);

CREATE INDEX IF NOT EXISTS ix_module_packages_module ON module_packages(module_id, version);
