// `/recall <query>` (issue #69) - "what did I note about X" from the bot.
//
// Honest boundary: the REAL hybrid recall (FTS5 + memvec/sqlite-vec, fused
// with RRF) already exists at `services/lifeos-api/src/routes/search.rs`
// (`GET /api/search`), but it queries `lifeos-derived.db` - an intentionally
// un-synced, Mac-local file (docs/DATA-MODEL.md §5) - and memvec.py is a
// Python subprocess. Neither is reachable from a Cloudflare Worker (no
// filesystem, and the Mac API only binds 127.0.0.1). So this is a lexical
// fallback, not the hybrid: a case-insensitive substring match over `title`
// and the raw `attrs` JSON in the canonical Turso DB, workspace-scoped,
// citing the matched entity. It works with the laptop off, same as every
// other bot command - the tradeoff is recall quality, not availability.
import { entities } from "@lifeos/db";
import { and, desc, eq, like, or } from "@lifeos/db/query";
import type { WorkerDb } from "@lifeos/db/client/worker";
import type { Entity } from "./entities.js";

export async function recallEntities(db: WorkerDb, workspaceId: string, query: string, limit = 5): Promise<Entity[]> {
  const needle = `%${query.trim()}%`;
  if (query.trim().length === 0) return [];

  const rows = await db
    .select()
    .from(entities)
    .where(and(eq(entities.workspaceId, workspaceId), or(like(entities.title, needle), like(entities.attrs, needle))))
    .orderBy(desc(entities.createdAt))
    .limit(limit);

  return rows as Entity[];
}
