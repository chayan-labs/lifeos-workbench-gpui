// Binds the Worker to the canonical Turso/libSQL DB via @lifeos/db's HTTP
// transport - issue #64 (docs/ARCHITECTURE.md §3.1, docs/DATA-MODEL.md).
// Workers have no filesystem/raw sockets, so this is the *only* DB access
// path available here (contrast db/client.mac.ts's embedded replica).
import { createWorkerDb, type WorkerDb } from "@lifeos/db/client/worker";

export function createDb(env: { TURSO_URL: string; TURSO_TOKEN: string }): WorkerDb {
  return createWorkerDb({ url: env.TURSO_URL, authToken: env.TURSO_TOKEN });
}

// Mirrors services/lifeos-api/src/config.rs's DEFAULT_WORKSPACE - both tiers
// must agree on the personal-workspace id so entities the bot writes are the
// same rows the SPA/API read, until real multi-user auth ties a Telegram
// chat to a specific workspace_id (SaaS hardening, phase 7).
export const DEFAULT_WORKSPACE = "default-personal-workspace";

export function resolveWorkspaceId(env: { WORKSPACE_ID?: string }): string {
  return env.WORKSPACE_ID && env.WORKSPACE_ID.length > 0 ? env.WORKSPACE_ID : DEFAULT_WORKSPACE;
}
