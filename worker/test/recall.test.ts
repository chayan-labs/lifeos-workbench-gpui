import { beforeEach, describe, expect, it } from "vitest";
import type { LocalDb } from "@lifeos/db/client/local";
import { createEntity } from "../src/entities.js";
import { recallEntities } from "../src/recall.js";
import { createTestDb } from "./testDb.js";

let db: LocalDb;
const WS = "ws_test";

beforeEach(async () => {
  db = await createTestDb();
});

describe("recallEntities", () => {
  it("matches a title substring, case-insensitively", async () => {
    await createEntity(db, WS, { module: "learning", type: "topic", title: "The Halting Problem" });
    await createEntity(db, WS, { module: "tasks", type: "task", title: "buy milk" });

    const hits = await recallEntities(db, WS, "halting");

    expect(hits).toHaveLength(1);
    expect(hits[0].title).toBe("The Halting Problem");
  });

  it("matches inside attrs when the title doesn't contain the query", async () => {
    await createEntity(db, WS, { module: "bot", type: "draft", attrs: { text: "announce the launch on Tuesday" } });

    const hits = await recallEntities(db, WS, "tuesday");

    expect(hits).toHaveLength(1);
  });

  it("never returns another workspace's entities", async () => {
    await createEntity(db, "ws_other", { module: "learning", type: "topic", title: "shared secret topic" });

    expect(await recallEntities(db, WS, "shared secret")).toHaveLength(0);
  });

  it("returns nothing for an empty query", async () => {
    expect(await recallEntities(db, WS, "   ")).toHaveLength(0);
  });
});
