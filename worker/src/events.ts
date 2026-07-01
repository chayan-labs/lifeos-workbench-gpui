// Read-only event queries - issue #65's `/pnl`. Trading is read-only for any
// agent/bot in this repo (docs/ARCHITECTURE.md hard rules: no order tool is
// ever registered) - summing `trade.closed` events is a pure read, never a
// broker call, so it needs no gate.
import { events } from "@lifeos/db";
import { and, eq } from "@lifeos/db/query";
import type { WorkerDb } from "@lifeos/db/client/worker";

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
