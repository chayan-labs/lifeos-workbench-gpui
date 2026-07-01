import { beforeEach, describe, expect, it } from "vitest";
import { createLocalDb, type LocalDb } from "@lifeos/db/client/local";
import { sql } from "@lifeos/db/query";
import { createEntity, listEntities } from "../src/entities.js";

// migrations/0001_core.sql's `entities`/`workspaces` DDL, trimmed to what
// these tests touch - a real in-memory SQLite DB via @lifeos/db's own
// drizzle-orm instance (see db/client.local.ts), not a mock.
const SCHEMA_SQL = `
CREATE TABLE workspaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  plan TEXT DEFAULT 'free',
  limits TEXT NOT NULL DEFAULT '{}',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE entities (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  module TEXT NOT NULL,
  type TEXT NOT NULL,
  parent_id TEXT,
  title TEXT,
  status TEXT,
  tier TEXT,
  attrs TEXT NOT NULL DEFAULT '{}',
  source TEXT,
  blob_ref TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  due INTEGER GENERATED ALWAYS AS (json_extract(attrs, '$.due')) VIRTUAL
);
`;

let db: LocalDb;

beforeEach(async () => {
  db = createLocalDb("file::memory:");
  for (const stmt of SCHEMA_SQL.split(";").map((s) => s.trim()).filter(Boolean)) {
    await db.run(sql.raw(stmt));
  }
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
