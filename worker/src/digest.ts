// Daily/scheduled digest (issue #71): due tasks, blocked items, realized PnL,
// and drafts awaiting approval. Pure content builder over the existing,
// already-tested command functions - no Telegram call here, so it's fully
// unit-testable against a local DB. `index.ts`'s `scheduled` handler is the
// thin, network-touching glue that actually sends it, same reason
// `/telegram` isn't unit-tested at that layer (worker/test/index.test.ts).
import type { WorkerDb } from "@lifeos/db/client/worker";
import { listPendingApprovals } from "./approvals.js";
import { formatPendingApproval, inbox, pnl, today } from "./commands.js";

export async function buildDigest(db: WorkerDb, workspaceId: string, nowSecs: number): Promise<string> {
  const [dueToday, blocked, realizedPnl, pending] = await Promise.all([
    today(db, workspaceId, nowSecs),
    // No `task.blocked` event exists yet - "uncategorized captures" is the
    // closest analog to "blocked items" until one does, same note as
    // docs/PLATFORM-SYSTEMS.md §3's #65 entry.
    inbox(db, workspaceId),
    pnl(db, workspaceId),
    listPendingApprovals(db, workspaceId),
  ]);

  const pendingSection = pending.length === 0 ? "Nothing pending approval." : pending.map(formatPendingApproval).join("\n");

  return ["Daily digest", "", "Due today:", dueToday, "", "Inbox (uncategorized / blocked):", blocked, "", realizedPnl, "", "Pending approval:", pendingSection].join("\n");
}
