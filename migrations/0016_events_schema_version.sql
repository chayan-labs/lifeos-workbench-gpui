-- 0016_events_schema_version.sql - versioned replay (issue #111,
-- docs/AI-MEMORY.md §2). Every event carries the schema version its payload
-- was written under; replay applies versioned upcasters so old events stay
-- replayable forever. Pre-existing rows default to 1 (the current shape).
-- Guarded by add_column_if_missing in db.rs.
ALTER TABLE events ADD COLUMN schema_version INTEGER NOT NULL DEFAULT 1;
