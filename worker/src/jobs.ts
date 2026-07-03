// Heavy/Mac-dependent work queue - issue #66. Mirrors the `module_requests`
// "Mac offline (queued)" pattern (README.md's Build order §4/§5): the Worker
// never executes a provider call itself (it holds no provider tokens), it
// only enqueues a `jobs` row; a LaunchAgent poller on the Mac
// (services/lifeos-drain) claims and dispatches it when awake. Real
// dispatch of `execute_approval` jobs (calling Nango's proxy / the browser
// actuator / trade-exec for the approved entity) is not built yet -
// lifeos-drain's other job kinds are stubs too (services/lifeos-drain/src/
// lib.rs's `dispatch()`) - out of scope for the bot side of the gate.
import { jobs } from "@lifeos/db";
import type { WorkerDb } from "@lifeos/db/client/worker";
import { ulid } from "ulid";

export async function enqueueJob(
  db: WorkerDb,
  workspaceId: string,
  kind: string,
  payload: Record<string, unknown>,
): Promise<string> {
  const id = `job_${ulid()}`;
  await db.insert(jobs).values({
    id,
    workspaceId,
    kind,
    payload: JSON.stringify(payload),
    status: "queued",
    priority: 0,
    createdAt: Math.floor(Date.now() / 1000),
  });
  return id;
}
