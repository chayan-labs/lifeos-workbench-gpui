// Workspace-scoped entity repository - issues #64/#65. Every query here
// takes a `workspaceId` and filters by it; callers must derive that id from
// `resolveWorkspaceId(env)` (db.ts), never from untrusted Telegram input, so
// the bot can never read/write another workspace's rows.
import { entities } from "@lifeos/db";
import { and, desc, eq, isNull, lte, or } from "@lifeos/db/query";
import type { WorkerDb } from "@lifeos/db/client/worker";
import { ulid } from "ulid";

export interface Entity {
  id: string;
  workspaceId: string;
  module: string;
  type: string;
  parentId: string | null;
  title: string | null;
  status: string | null;
  tier: string | null;
  attrs: string;
  source: string | null;
  blobRef: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface ListEntitiesOptions {
  module?: string;
  type?: string;
  status?: string;
  limit?: number;
}

export async function listEntities(
  db: WorkerDb,
  workspaceId: string,
  opts: ListEntitiesOptions = {},
): Promise<Entity[]> {
  const conditions = [eq(entities.workspaceId, workspaceId)];
  if (opts.module) conditions.push(eq(entities.module, opts.module));
  if (opts.type) conditions.push(eq(entities.type, opts.type));
  if (opts.status) conditions.push(eq(entities.status, opts.status));

  const rows = await db
    .select()
    .from(entities)
    .where(and(...conditions))
    .orderBy(desc(entities.createdAt))
    .limit(opts.limit ?? 50);

  return rows as Entity[];
}

export interface CreateEntityInput {
  module: string;
  type: string;
  title?: string;
  status?: string;
  attrs?: Record<string, unknown>;
  source?: string;
}

export async function createEntity(
  db: WorkerDb,
  workspaceId: string,
  input: CreateEntityInput,
): Promise<Entity> {
  const now = Math.floor(Date.now() / 1000); // *_at columns are Unix seconds (services/lifeos-api/src/ids.rs)
  const row = {
    id: `ent_${ulid()}`,
    workspaceId,
    module: input.module,
    type: input.type,
    parentId: null,
    title: input.title ?? null,
    status: input.status ?? null,
    tier: null,
    attrs: JSON.stringify(input.attrs ?? {}),
    source: input.source ?? null,
    blobRef: null,
    createdAt: now,
    updatedAt: now,
  };

  await db.insert(entities).values(row);
  return row as Entity;
}

// `/today` (issue #65): open tasks with no due date, or due on/before the
// given cutoff (Unix seconds, typically end-of-today). No natural-language
// due-date parsing on capture yet (`/task` never sets `attrs.due`) - this is
// intentionally naive, same "real but simple, not AI-powered" precedent as
// reading.rs's naive_summary. Sorted client-side (undated tasks last) since
// SQLite's NULLS LAST ordering isn't worth a raw-SQL escape hatch here.
export async function listOpenTasksDueBy(db: WorkerDb, workspaceId: string, cutoff: number): Promise<Entity[]> {
  const rows = (await db
    .select()
    .from(entities)
    .where(
      and(
        eq(entities.workspaceId, workspaceId),
        eq(entities.module, "tasks"),
        eq(entities.type, "task"),
        eq(entities.status, "open"),
        or(isNull(entities.due), lte(entities.due, cutoff)),
      ),
    )
    .limit(50)) as (Entity & { due: number | null })[];

  return rows.sort((a, b) => (a.due ?? Infinity) - (b.due ?? Infinity));
}

// `/inbox` (issue #65): recently captured entities nothing has triaged yet -
// defined here as "no status set" (a plain-captured `/topic`, for example;
// `/task` sets status='open' immediately so it never shows up as inbox).
export async function listInbox(db: WorkerDb, workspaceId: string, limit = 10): Promise<Entity[]> {
  const rows = await db
    .select()
    .from(entities)
    .where(and(eq(entities.workspaceId, workspaceId), isNull(entities.status)))
    .orderBy(desc(entities.createdAt))
    .limit(limit);

  return rows as Entity[];
}

export type MarkTaskDoneResult = { outcome: "done"; entity: Entity } | { outcome: "not_found" } | { outcome: "ambiguous" };

// `/done <id-suffix>` (issue #65): matches by the tail of the ULID rather
// than requiring the full id, since that's what's practical to copy off a
// phone from a `/today` listing (bot.ts truncates ids the same way).
export async function markTaskDoneBySuffix(db: WorkerDb, workspaceId: string, suffix: string): Promise<MarkTaskDoneResult> {
  const openTasks = (await listEntities(db, workspaceId, { module: "tasks", type: "task", status: "open", limit: 500 })) as Entity[];
  const matches = openTasks.filter((t) => t.id.toLowerCase().endsWith(suffix.toLowerCase()));

  if (matches.length === 0) return { outcome: "not_found" };
  if (matches.length > 1) return { outcome: "ambiguous" };

  const [task] = matches;
  const updatedAt = Math.floor(Date.now() / 1000);
  await db.update(entities).set({ status: "done", updatedAt }).where(and(eq(entities.workspaceId, workspaceId), eq(entities.id, task.id)));

  return { outcome: "done", entity: { ...task, status: "done", updatedAt } };
}
