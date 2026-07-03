// Event queries - issues #65 (`/pnl`, read-only) and #66 (recording every
// approve/deny transition; `events` is append-only, docs/ARCHITECTURE.md
// hard rules, so this never updates/deletes a row).
import { events } from "@lifeos/db";
import { and, eq } from "@lifeos/db/query";
import type { WorkerDb } from "@lifeos/db/client/worker";
import { ulid } from "ulid";

export async function sumClosedTradePnl(db: WorkerDb, workspaceId: string): Promise<number> {
  const rows = await db
    .select({ attrs: events.attrs })
    .from(events)
    .where(and(eq(events.workspaceId, workspaceId), eq(events.type, "trade.closed")));

  return rows.reduce((total, row) => {
    const attrs = JSON.parse(row.attrs ?? "{}") as { pnl?: number };
    return total + (typeof attrs.pnl === "number" ? attrs.pnl : 0);
  }, 0);
}

export async function recordEvent(
  db: WorkerDb,
  workspaceId: string,
  type: string,
  entityId: string | null,
  attrs: Record<string, unknown> = {},
): Promise<void> {
  await db.insert(events).values({
    id: `evt_${ulid()}`,
    workspaceId,
    ts: Math.floor(Date.now() / 1000),
    type,
    entityId,
    actor: "bot",
    attrs: JSON.stringify(attrs),
  });
}
