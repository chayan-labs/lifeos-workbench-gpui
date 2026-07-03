// Local-only libSQL binding: no Turso, no sync - for tests and quick local
// dev only. NOT the Mac's local-first tier (that's client.mac.ts, which
// syncs against a Turso primary via `syncUrl`). Consumers testing
// @lifeos/db-based query code (e.g. worker/test/entities.test.ts) use this
// to spin up a throwaway in-memory or file-backed SQLite DB against the
// identical schema and drizzle-orm instance this package ships, avoiding
// the branded-type mismatch a second independently-installed drizzle-orm
// copy would cause (see query.ts).
import { createClient } from "@libsql/client";
import { drizzle } from "drizzle-orm/libsql";
import * as schema from "./schema.js";

export function createLocalDb(url: string = "file::memory:") {
  const client = createClient({ url });
  return drizzle(client, { schema });
}

export type LocalDb = ReturnType<typeof createLocalDb>;
