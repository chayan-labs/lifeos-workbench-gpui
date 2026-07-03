import { beforeEach, describe, expect, it } from "vitest";
import type { LocalDb } from "@lifeos/db/client/local";
import { moduleRequests } from "@lifeos/db";
import { eq } from "@lifeos/db/query";
import { enqueueModuleRequest } from "../src/moduleRequests.js";
import { createTestDb } from "./testDb.js";

let db: LocalDb;

beforeEach(async () => {
  db = await createTestDb();
});

describe("enqueueModuleRequest", () => {
  it("writes a queued module_requests row for the workspace", async () => {
    await enqueueModuleRequest(db, "ws_a", "add a health tracker");

    const rows = await db.select().from(moduleRequests).where(eq(moduleRequests.workspaceId, "ws_a"));

    expect(rows).toHaveLength(1);
    expect(rows[0].status).toBe("queued");
    expect(rows[0].prompt).toBe("add a health tracker");
  });

  it("persists the requester's chat_id so lifeos-drain can notify them back (issue #78)", async () => {
    await enqueueModuleRequest(db, "ws_a", "add a health tracker", "12345");

    const rows = await db.select().from(moduleRequests).where(eq(moduleRequests.workspaceId, "ws_a"));

    expect(rows[0].chatId).toBe("12345");
  });

  it("leaves chat_id null when the caller doesn't supply one", async () => {
    await enqueueModuleRequest(db, "ws_a", "add a health tracker");

    const rows = await db.select().from(moduleRequests).where(eq(moduleRequests.workspaceId, "ws_a"));

    expect(rows[0].chatId).toBeNull();
  });

  it("never touches another workspace's requests", async () => {
    await enqueueModuleRequest(db, "ws_a", "a's request");
    await enqueueModuleRequest(db, "ws_b", "b's request");

    const rowsForA = await db.select().from(moduleRequests).where(eq(moduleRequests.workspaceId, "ws_a"));

    expect(rowsForA).toHaveLength(1);
    expect(rowsForA[0].prompt).toBe("a's request");
  });
});
