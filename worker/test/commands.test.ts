import { beforeEach, describe, expect, it } from "vitest";
import type { LocalDb } from "@lifeos/db/client/local";
import { events } from "@lifeos/db";
import { captureDraft, captureTask, captureTopic, inbox, markDone, pnl, quiz, today } from "../src/commands.js";
import { listEntities } from "../src/entities.js";
import { createTestDb } from "./testDb.js";

let db: LocalDb;
const WS = "ws_test";

beforeEach(async () => {
  db = await createTestDb();
});

describe("captureTask", () => {
  it("creates an open task and confirms it", async () => {
    const reply = await captureTask(db, WS, "buy milk");

    expect(reply).toMatch(/^Task captured \[\w+\]: buy milk$/);
    const rows = await listEntities(db, WS, { module: "tasks", type: "task" });
    expect(rows).toHaveLength(1);
    expect(rows[0].status).toBe("open");
  });

  it("rejects an empty task", async () => {
    expect(await captureTask(db, WS, "   ")).toMatch(/^Usage:/);
  });
});

describe("captureTopic", () => {
  it("creates an uncategorized topic (shows up in the inbox)", async () => {
    await captureTopic(db, WS, "quantum computing");

    expect(await inbox(db, WS)).toContain("quantum computing");
  });
});

describe("captureDraft", () => {
  it("creates a pending_approval draft, never publishing anything", async () => {
    const reply = await captureDraft(db, WS, "post about the launch");

    expect(reply).toMatch(/awaiting approval/);
    const rows = await listEntities(db, WS, { module: "bot", type: "draft" });
    expect(rows[0].status).toBe("pending_approval");
  });
});

describe("markDone", () => {
  it("completes a task captured moments earlier, addressed by its short id", async () => {
    const captureReply = await captureTask(db, WS, "ship it");
    const shortId = captureReply.match(/\[(\w+)\]/)?.[1] ?? "";

    const doneReply = await markDone(db, WS, shortId);

    expect(doneReply).toBe("Done: ship it");
  });

  it("reports no match for an unknown id", async () => {
    expect(await markDone(db, WS, "nomatch")).toMatch(/No open task/);
  });
});

describe("today", () => {
  it("lists undated open tasks", async () => {
    await captureTask(db, WS, "no due date yet");

    expect(await today(db, WS, 1_000)).toContain("no due date yet");
  });

  it("says nothing due when there are no open tasks", async () => {
    expect(await today(db, WS, 1_000)).toBe("Nothing due today.");
  });
});

describe("inbox", () => {
  it("is empty until something is captured without a status", async () => {
    expect(await inbox(db, WS)).toBe("Inbox is empty.");
    await captureTopic(db, WS, "first topic");
    expect(await inbox(db, WS)).toContain("first topic");
  });
});

describe("pnl", () => {
  it("reports the realized total from trade.closed events", async () => {
    await db.insert(events).values({
      id: "evt_1",
      workspaceId: WS,
      ts: 1,
      type: "trade.closed",
      attrs: JSON.stringify({ pnl: 42.5 }),
    });

    expect(await pnl(db, WS)).toBe("All-time realized PnL: +42.50");
  });
});

describe("quiz", () => {
  it("prompts about a captured topic", async () => {
    await captureTopic(db, WS, "the halting problem");

    expect(await quiz(db, WS)).toContain("the halting problem");
  });

  it("tells you to capture a topic first when there are none", async () => {
    expect(await quiz(db, WS)).toMatch(/No topics/);
  });
});
