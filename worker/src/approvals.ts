// The gating state machine surfaced in Telegram - issue #66
// (docs/SECURITY.md §2): pending_approval -> approved|denied, every
// transition an event, approval only ever enqueues work for the Mac to
// execute - the Worker itself never calls Nango's proxy, the browser
// actuator, or trade-exec directly (it holds no provider tokens,
// docs/ARCHITECTURE.md §3.1). Real dispatch of the enqueued
// `execute_approval` job is services/lifeos-drain's job (not built yet -
// its other job kinds are stubs too), so "on approve" here means "queued for
// execution," not "executed."
import type { WorkerDb } from "@lifeos/db/client/worker";
import { type Entity, getEntityById, listEntities, transitionEntityStatus } from "./entities.js";
import { recordEvent } from "./events.js";
import { enqueueJob } from "./jobs.js";

export const PENDING_APPROVAL_STATUS = "pending_approval";

export async function listPendingApprovals(db: WorkerDb, workspaceId: string, limit = 10): Promise<Entity[]> {
  return listEntities(db, workspaceId, { status: PENDING_APPROVAL_STATUS, limit });
}

export type ApprovalResult =
  | { outcome: "approved"; entity: Entity }
  | { outcome: "denied"; entity: Entity }
  | { outcome: "not_found" }
  | { outcome: "already_resolved"; entity: Entity };

async function resolveOrAlreadyResolved(
  db: WorkerDb,
  workspaceId: string,
  id: string,
  toStatus: "approved" | "denied",
): Promise<ApprovalResult> {
  const existing = await getEntityById(db, workspaceId, id);
  if (!existing) return { outcome: "not_found" };
  if (existing.status !== PENDING_APPROVAL_STATUS) return { outcome: "already_resolved", entity: existing };

  const updated = await transitionEntityStatus(db, workspaceId, id, PENDING_APPROVAL_STATUS, toStatus);
  // A concurrent tap could win the race between the check above and the
  // conditional UPDATE - treat that as already_resolved too, not a crash.
  if (!updated) return { outcome: "already_resolved", entity: existing };

  return { outcome: toStatus, entity: updated } as ApprovalResult;
}

export async function approveEntity(db: WorkerDb, workspaceId: string, id: string): Promise<ApprovalResult> {
  const result = await resolveOrAlreadyResolved(db, workspaceId, id, "approved");
  if (result.outcome !== "approved") return result;

  await recordEvent(db, workspaceId, `${result.entity.type}.approved`, id);
  await enqueueJob(db, workspaceId, "execute_approval", { entity_id: id, entity_type: result.entity.type });

  return result;
}

export async function denyEntity(db: WorkerDb, workspaceId: string, id: string): Promise<ApprovalResult> {
  const result = await resolveOrAlreadyResolved(db, workspaceId, id, "denied");
  if (result.outcome !== "denied") return result;

  // docs/SECURITY.md §2's exact naming for the deny transition.
  await recordEvent(db, workspaceId, `${result.entity.type}.rejected`, id);

  return result;
}
