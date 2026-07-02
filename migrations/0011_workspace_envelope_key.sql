-- Per-workspace envelope key (issue #104, docs/SECURITY.md §5) - AES-256-GCM
-- key generated at first provision, itself encrypted under the server's
-- master LIFEOS_SECRET_ENCRYPTION_KEY. Nullable: not every workspace
-- provisions a dedicated database.
ALTER TABLE workspaces ADD COLUMN envelope_key_enc TEXT;
