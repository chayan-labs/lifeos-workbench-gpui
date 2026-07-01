// Workspace-scoped entity repository - issue #64. Every query here takes a
// `workspaceId` and filters by it; callers must derive that id from
// `resolveWorkspaceId(env)` (db.ts), never from untrusted Telegram input, so
// the bot can never read/write another workspace's rows.
import { entities } from "@lifeos/db";
import { and, desc, eq } from "@lifeos/db/query";
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
