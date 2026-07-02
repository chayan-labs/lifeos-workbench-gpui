-- Real login (issue #100, docs/SECURITY.md §5): password_hash on users.
-- Nullable - the seeded personal user and any pre-#100 row has none, so
-- login for those accounts fails closed (rejected, not a crash) until a
-- password is set via register/reset.

ALTER TABLE users ADD COLUMN password_hash TEXT;
