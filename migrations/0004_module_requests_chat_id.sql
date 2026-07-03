-- Requester identity for `module_requests` (issue #78): the Telegram chat to
-- notify on install/failure. Nullable - API-originated requests (no chat
-- behind them) leave this NULL and simply aren't notified.

ALTER TABLE module_requests ADD COLUMN chat_id TEXT;
