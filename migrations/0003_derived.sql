-- 0003_derived.sql - schema for the NEVER-SYNCED derived DB (attached as `d`).
--
-- This holds search/recall state only; it is rebuildable from the canonical DB
-- at any time (DATA-MODEL §5-6). Physically separate file => can never be
-- pushed to the Turso primary. Applied to the attached schema `d` at boot.
--
-- Lexical half (FTS5) lives here and is fully owned by lifeos-api - FTS5 is
-- compiled into libSQL core, no extension needed. The semantic half
-- (`entity_vec`, sqlite-vec `vec0`) is owned by server/memvec.py, because the
-- vec0 loadable extension is NOT available to the Rust libSQL build.

-- Projection of canonical entities, the searchable source-of-truth for FTS.
-- `attrs_text` is the FLATTENED form of the attrs JSON (braces/quotes/colons
-- collapsed to spaces) so FTS5 tokenizes the keys and values. It is a STORED
-- generated column, so it can never drift from `attrs`.
CREATE TABLE IF NOT EXISTS entities_idx (
    rowid        INTEGER PRIMARY KEY,
    id           TEXT NOT NULL UNIQUE,
    workspace_id TEXT NOT NULL,
    module       TEXT NOT NULL,
    type         TEXT NOT NULL,
    title        TEXT,
    status       TEXT,
    attrs        TEXT NOT NULL DEFAULT '{}',
    attrs_text   TEXT GENERATED ALWAYS AS (
        replace(replace(replace(replace(replace(replace(
            attrs, '{', ' '), '}', ' '), '"', ' '), ':', ' '), ',', ' '), '[', ' ')
    ) STORED,
    updated_at   INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_entities_idx_ws ON entities_idx (workspace_id);

-- FTS5 index over the projection (external-content: rows mirror entities_idx by
-- rowid). porter stemming + unicode61 so "theorem"/"theorems" both hit.
CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(
    title,
    attrs_text,
    module,
    type,
    content='entities_idx',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Triggers keep the FTS index in lockstep with the projection table.
CREATE TRIGGER IF NOT EXISTS entities_idx_ai AFTER INSERT ON entities_idx BEGIN
    INSERT INTO entities_fts(rowid, title, attrs_text, module, type)
    VALUES (new.rowid, new.title, new.attrs_text, new.module, new.type);
END;

CREATE TRIGGER IF NOT EXISTS entities_idx_ad AFTER DELETE ON entities_idx BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, title, attrs_text, module, type)
    VALUES ('delete', old.rowid, old.title, old.attrs_text, old.module, old.type);
END;

CREATE TRIGGER IF NOT EXISTS entities_idx_au AFTER UPDATE ON entities_idx BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, title, attrs_text, module, type)
    VALUES ('delete', old.rowid, old.title, old.attrs_text, old.module, old.type);
    INSERT INTO entities_fts(rowid, title, attrs_text, module, type)
    VALUES (new.rowid, new.title, new.attrs_text, new.module, new.type);
END;
