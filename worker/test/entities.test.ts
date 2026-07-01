import { beforeEach, describe, expect, it } from "vitest";
import type { LocalDb } from "@lifeos/db/client/local";
import { createEntity, listEntities, listInbox, listOpenTasksDueBy, markTaskDoneBySuffix } from "../src/entities.js";
import { createTestDb } from "./testDb.js";

let db: LocalDb;

beforeEach(async () => {
  db = await createTestDb();
});

describe("createEntity + listEntities", () => {
  it("round-trips a created entity back through list", async () => {
    const created = await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "write tests" });

    const rows = await listEntities(db, "ws_a");

    expect(rows).toHaveLength(1);
    expect(rows[0].id).toBe(created.id);
    expect(rows[0].title).toBe("write tests");
    expect(rows[0].workspaceId).toBe("ws_a");
  });

  it("filters by module and type", async () => {
    await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "a" });
    await createEntity(db, "ws_a", { module: "reading", type: "article", title: "b" });

    const rows = await listEntities(db, "ws_a", { module: "tasks", type: "task" });

    expect(rows).toHaveLength(1);
    expect(rows[0].title).toBe("a");
  });

  it("never returns another workspace's entities", async () => {
    await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "a's task" });
    await createEntity(db, "ws_b", { module: "tasks", type: "task", title: "b's task" });

    const rowsForA = await listEntities(db, "ws_a");
    const rowsForB = await listEntities(db, "ws_b");

    expect(rowsForA).toHaveLength(1);
    expect(rowsForA[0].title).toBe("a's task");
    expect(rowsForB).toHaveLength(1);
    expect(rowsForB[0].title).toBe("b's task");
  });
});

describe("listOpenTasksDueBy", () => {
  it("includes undated open tasks and excludes tasks due after the cutoff", async () => {
    await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "no due date", status: "open" });
    await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "due later", status: "open", attrs: { due: 2_000_000_000 } });
    await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "done already", status: "done" });

    const rows = await listOpenTasksDueBy(db, "ws_a", 1_000_000_000);

    expect(rows.map((r) => r.title)).toEqual(["no due date"]);
  });

  it("sorts dated tasks before undated ones", async () => {
    await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "no due date", status: "open" });
    await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "due soon", status: "open", attrs: { due: 100 } });

    const rows = await listOpenTasksDueBy(db, "ws_a", 1_000_000_000);

    expect(rows.map((r) => r.title)).toEqual(["due soon", "no due date"]);
  });
});

describe("listInbox", () => {
  it("returns only entities with no status, most recent first", async () => {
    await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "triaged", status: "open" });
    await createEntity(db, "ws_a", { module: "learning", type: "topic", title: "raw capture" });

    const rows = await listInbox(db, "ws_a");

    expect(rows.map((r) => r.title)).toEqual(["raw capture"]);
  });
});

describe("markTaskDoneBySuffix", () => {
  it("marks the matching open task done", async () => {
    const created = await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "ship it", status: "open" });
    const suffix = created.id.slice(-6);

    const result = await markTaskDoneBySuffix(db, "ws_a", suffix);

    expect(result.outcome).toBe("done");
    const rows = await listEntities(db, "ws_a", { module: "tasks", type: "task" });
    expect(rows[0].status).toBe("done");
  });

  it("reports not_found for no match", async () => {
    const result = await markTaskDoneBySuffix(db, "ws_a", "zzzzzz");
    expect(result.outcome).toBe("not_found");
  });

  it("reports ambiguous when a suffix matches more than one open task", async () => {
    await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "first", status: "open" });
    await createEntity(db, "ws_a", { module: "tasks", type: "task", title: "second", status: "open" });

    // An empty suffix trivially matches every id (String.endsWith("") is
    // always true) - a deterministic way to force the multi-match branch
    // without relying on two random ULIDs sharing a real tail.
    const result = await markTaskDoneBySuffix(db, "ws_a", "");

    expect(result.outcome).toBe("ambiguous");
  });
});
