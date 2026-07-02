-- 0014_blob_backends.sql - backend index for pluggable storage backends
-- (issue #106, docs/STORAGE-BACKENDS.md §2).
--
-- Maps a content hash (blob_ref / chunk hash) to the backend-native locator
-- on each configured backend. Path-shaped backends derive locators from the
-- hash; id-shaped backends (Google Drive / Dropbox file ids) need this index
-- to resolve a hash at all. Metadata only - blob BYTES never enter libSQL
-- (docs/DATA-MODEL.md §4).

CREATE TABLE IF NOT EXISTS blob_backends (
  workspace_id TEXT NOT NULL,
  backend_id   TEXT NOT NULL,             -- storage_backend config entity id
  hash         TEXT NOT NULL,             -- BLAKE3 blob_ref or chunk hash
  locator      TEXT NOT NULL,             -- backend-native path / object key / file id
  created_at   INTEGER NOT NULL,
  PRIMARY KEY (workspace_id, backend_id, hash),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);

CREATE INDEX IF NOT EXISTS ix_blob_backends_hash ON blob_backends(workspace_id, hash);
