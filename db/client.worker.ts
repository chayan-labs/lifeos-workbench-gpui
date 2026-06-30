// Cloudflare Worker binding: HTTP-only transport, no filesystem/sockets available.
// See docs/DATA-MODEL.md §4 and CLAUDE.md "Three tiers, one DB".
import { createClient } from "@libsql/client/web";
import { drizzle } from "drizzle-orm/libsql";
import * as schema from "./schema.js";

export type WorkerDbConfig = {
  url: string; // Turso primary URL (libsql://...)
  authToken: string;
};

export function createWorkerDb(config: WorkerDbConfig) {
  const client = createClient({
    url: config.url,
    authToken: config.authToken,
  });
  return drizzle(client, { schema });
}

export type WorkerDb = ReturnType<typeof createWorkerDb>;
