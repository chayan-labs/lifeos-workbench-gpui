-- 0015_events_caused_by.sql - causal pointer on the append-only event log
-- (issue #111, docs/AI-MEMORY.md §3). "What led to this?" - consolidation and
-- the memory projector follow this pointer to build caused_by memory edges.
-- Guarded by add_column_if_missing in db.rs (ALTER ADD COLUMN is not
-- naturally idempotent).
ALTER TABLE events ADD COLUMN caused_by_event_id TEXT;
