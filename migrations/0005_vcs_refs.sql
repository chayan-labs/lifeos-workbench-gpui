-- Branch/tag pointers for lifeos-vcs (issue #84, docs/VERSIONING.md §2.4).
-- A branch is a named, moving pointer to a snapshot manifest hash; a tag is
-- a fixed pointer (immutable once set). This is VCS infra state, not domain
-- data, so it's its own small table - same precedent as `jobs`/`module_requests`.

CREATE TABLE IF NOT EXISTS vcs_refs (
  workspace_id  TEXT NOT NULL,
  kind          TEXT NOT NULL, -- 'branch' | 'tag'
  name          TEXT NOT NULL,
  snapshot_ref  TEXT NOT NULL,
  updated_at    INTEGER NOT NULL,
  PRIMARY KEY (workspace_id, kind, name)
);
