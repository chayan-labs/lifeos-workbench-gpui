// Capture/query command logic - issue #65. Kept as plain functions over
// (db, workspaceId, ...args) -> reply string, separate from bot.ts's grammY
// wiring, so each command is testable without constructing a Context.
// Everything here is a "medium action" per the issue's scope (an entity/
// event read or write in the bot's own workspace) - nothing heavy/codegen
// is triggered from a command yet, so there is nothing to enqueue to `jobs`
// in this iteration.
import type { WorkerDb } from "@lifeos/db/client/worker";
import {
  type Entity,
  createEntity,
  listEntities,
  listInbox,
  listOpenTasksDueBy,
  markTaskDoneBySuffix,
} from "./entities.js";
import { sumClosedTradePnl } from "./events.js";

const SHORT_ID_LEN = 6;

function shortId(id: string): string {
  return id.slice(-SHORT_ID_LEN);
}

function endOfDayUtc(nowSecs: number): number {
  const startOfDay = Math.floor(nowSecs / 86400) * 86400;
  return startOfDay + 86400 - 1;
}

export async function captureTask(db: WorkerDb, workspaceId: string, text: string): Promise<string> {
  const title = text.trim();
  if (!title) return "Usage: /task <what needs doing>";

  const entity = await createEntity(db, workspaceId, { module: "tasks", type: "task", title, status: "open", source: "telegram" });
  return `Task captured [${shortId(entity.id)}]: ${title}`;
}

export async function captureTopic(db: WorkerDb, workspaceId: string, text: string): Promise<string> {
  const title = text.trim();
  if (!title) return "Usage: /topic <what to learn>";

  const entity = await createEntity(db, workspaceId, { module: "learning", type: "topic", title, source: "telegram" });
  return `Topic added to inbox [${shortId(entity.id)}]: ${title}`;
}

// Outward actions are human-gated (docs/ARCHITECTURE.md hard rules): this
// only ever creates a pending_approval row, exactly like every
// draft_action-backed route in services/lifeos-api. The Telegram-side
// approve/deny keyboard that acts on it lands in #66.
export async function captureDraft(db: WorkerDb, workspaceId: string, text: string): Promise<string> {
  const body = text.trim();
  if (!body) return "Usage: /draft <what to draft>";

  const entity = await createEntity(db, workspaceId, {
    module: "bot",
    type: "draft",
    status: "pending_approval",
    attrs: { text: body },
    source: "telegram",
  });
  return `Drafted [${shortId(entity.id)}], awaiting approval: ${body}`;
}

export async function markDone(db: WorkerDb, workspaceId: string, suffix: string): Promise<string> {
  const arg = suffix.trim();
  if (!arg) return "Usage: /done <task id>";

  const result = await markTaskDoneBySuffix(db, workspaceId, arg);
  if (result.outcome === "not_found") return `No open task matching "${arg}".`;
  if (result.outcome === "ambiguous") return `More than one open task matches "${arg}" - use more of the id.`;
  return `Done: ${result.entity.title}`;
}

export async function today(db: WorkerDb, workspaceId: string, nowSecs: number): Promise<string> {
  const tasks = await listOpenTasksDueBy(db, workspaceId, endOfDayUtc(nowSecs));
  if (tasks.length === 0) return "Nothing due today.";

  return tasks.map((t) => `[${shortId(t.id)}] ${t.title}`).join("\n");
}

export async function inbox(db: WorkerDb, workspaceId: string): Promise<string> {
  const rows = await listInbox(db, workspaceId);
  if (rows.length === 0) return "Inbox is empty.";

  return rows.map((e) => `[${shortId(e.id)}] (${e.module}/${e.type}) ${e.title ?? "(untitled)"}`).join("\n");
}

export async function pnl(db: WorkerDb, workspaceId: string): Promise<string> {
  const total = await sumClosedTradePnl(db, workspaceId);
  const sign = total >= 0 ? "+" : "";
  return `All-time realized PnL: ${sign}${total.toFixed(2)}`;
}

// Spaced-repetition prompt, naive: the learning/topic entity untouched the
// longest. No SM-2 scheduling / quality-rating loop yet (docs/MODULES.md's
// naive-but-real precedent, same as reading.rs's link_topics) - just enough
// to make `/quiz` do something real on top of `/topic` captures.
export async function quiz(db: WorkerDb, workspaceId: string): Promise<string> {
  const topics = await listEntities(db, workspaceId, { module: "learning", type: "topic", limit: 500 });
  if (topics.length === 0) return "No topics to quiz yet - add one with /topic.";

  const oldest = topics.reduce((least: Entity, t: Entity) => (t.updatedAt < least.updatedAt ? t : least));
  return `Quiz: what do you remember about "${oldest.title}"?`;
}
