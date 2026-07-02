-- Remove the billing/quota seam (issue #104). This is a self-hosted,
-- bring-your-own-database-and-AI-model project - there is no product to
-- meter or bill, so the 'plans'/'subscriptions' catalog from
-- 0002_control_plane.sql (never read by any route) is dropped rather than
-- built out. Tenancy/workspaces/auth are unaffected - only the billing seam
-- goes.
DROP TABLE IF EXISTS subscriptions;
DROP TABLE IF EXISTS plans;
