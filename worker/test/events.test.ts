import { beforeEach, describe, expect, it } from "vitest";
import type { LocalDb } from "@lifeos/db/client/local";
import { events } from "@lifeos/db";
import { sumClosedTradePnl } from "../src/events.js";
import { createTestDb } from "./testDb.js";

let db: LocalDb;

beforeEach(async () => {
  db = await createTestDb();
});

async function seedTradeClosed(workspaceId: string, pnlValue: number) {
  await db.insert(events).values({
    id: `evt_${crypto.randomUUID()}`,
    workspaceId,
    ts: 1,
    type: "trade.closed",
    attrs: JSON.stringify({ pnl: pnlValue }),
  });
}

describe("sumClosedTradePnl", () => {
  it("sums pnl across trade.closed events for the workspace", async () => {
    await seedTradeClosed("ws_a", 100);
    await seedTradeClosed("ws_a", -25.5);

    const total = await sumClosedTradePnl(db, "ws_a");

    expect(total).toBe(74.5);
  });

  it("ignores other event types and other workspaces", async () => {
    await seedTradeClosed("ws_a", 100);
    await seedTradeClosed("ws_b", 9999);
    await db.insert(events).values({
      id: `evt_${crypto.randomUUID()}`,
      workspaceId: "ws_a",
      ts: 1,
      type: "task.completed",
      attrs: JSON.stringify({ pnl: 5000 }),
    });

    const total = await sumClosedTradePnl(db, "ws_a");

    expect(total).toBe(100);
  });

  it("returns 0 when there are no closed trades", async () => {
    expect(await sumClosedTradePnl(db, "ws_a")).toBe(0);
  });
});
