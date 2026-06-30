// Mac embedded-replica binding: local-first reads/writes against lifeos.db,
// periodic background sync with the Turso primary.
// `offline: true` is REQUIRED - without it, writes go straight to the remote
// primary and the "local-first" guarantee breaks. See docs/DATA-MODEL.md §4.1.
import { createClient } from "@libsql/client";
import { drizzle } from "drizzle-orm/libsql";
import * as schema from "./schema.js";

export type MacDbConfig = {
  path: string; // e.g. "file:lifeos.db"
  syncUrl: string; // Turso primary URL
  authToken: string;
  syncIntervalSeconds?: number; // default 60
};

export function createMacDb(config: MacDbConfig) {
  const client = createClient({
    url: config.path,
    syncUrl: config.syncUrl,
    authToken: config.authToken,
    syncInterval: config.syncIntervalSeconds ?? 60,
    offline: true,
  });
  return drizzle(client, { schema });
}

export type MacDb = ReturnType<typeof createMacDb>;
