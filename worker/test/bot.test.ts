import { beforeEach, describe, expect, it } from "vitest";
import type { UserFromGetMe } from "grammy/types";
import type { LocalDb } from "@lifeos/db/client/local";
import { createBot, healthMessage, type BotDeps } from "../src/bot.js";
import { createTestDb } from "./testDb.js";

const FAKE_BOT_INFO: UserFromGetMe = {
  id: 1,
  is_bot: true,
  first_name: "Life OS",
  username: "lifeos_test_bot",
  can_join_groups: true,
  can_read_all_group_messages: false,
  supports_inline_queries: false,
  can_connect_to_business: false,
  has_main_web_app: false,
  has_topics_enabled: false,
  allows_users_to_create_topics: false,
  can_manage_bots: false,
  supports_join_request_queries: false,
};

const WS = "ws_test";

function textUpdate(text: string) {
  return {
    update_id: 1,
    message: {
      message_id: 1,
      date: 0,
      chat: { id: 1, type: "private" as const, first_name: "tester" },
      from: { id: 1, is_bot: false, first_name: "tester" },
      text,
      entities: [{ offset: 0, length: text.split(" ")[0].length, type: "bot_command" as const }],
    },
  };
}

// grammY's documented offline-testing pattern: pass `botInfo` so `bot.init()`
// never calls Telegram's getMe, and intercept outgoing API calls via
// `bot.api.config.use` instead of hitting the network.
function repliesFrom(bot: ReturnType<typeof createBot>) {
  const sent: string[] = [];
  bot.api.config.use((prev, method, payload, signal) => {
    if (method === "sendMessage" && typeof (payload as { text?: string }).text === "string") {
      sent.push((payload as { text: string }).text);
      return Promise.resolve({ ok: true, result: {} } as never);
    }
    return prev(method, payload, signal);
  });
  return sent;
}

let db: LocalDb;
let deps: BotDeps;

beforeEach(async () => {
  db = await createTestDb();
  deps = { token: "fake-token", db, workspaceId: WS };
});

describe("healthMessage", () => {
  it("returns a fixed ok string", () => {
    expect(healthMessage()).toBe("ok");
  });
});

describe("createBot - scaffold commands", () => {
  it("replies to /start with an online message", async () => {
    const bot = createBot(deps, FAKE_BOT_INFO);
    const sent = repliesFrom(bot);

    await bot.init();
    await bot.handleUpdate(textUpdate("/start"));

    expect(sent).toEqual(["Life OS bot is online."]);
  });

  it("replies to /health with the health message", async () => {
    const bot = createBot(deps, FAKE_BOT_INFO);
    const sent = repliesFrom(bot);

    await bot.init();
    await bot.handleUpdate(textUpdate("/health"));

    expect(sent).toEqual([healthMessage()]);
  });

  it("does not reply to unrelated text", async () => {
    const bot = createBot(deps, FAKE_BOT_INFO);
    const sent = repliesFrom(bot);

    await bot.init();
    await bot.handleUpdate({
      update_id: 2,
      message: {
        message_id: 2,
        date: 0,
        chat: { id: 1, type: "private" as const, first_name: "tester" },
        from: { id: 1, is_bot: false, first_name: "tester" },
        text: "hello there",
      },
    });

    expect(sent).toEqual([]);
  });
});

describe("createBot - capture/query commands (issue #65)", () => {
  it("captures a task via /task and lists it via /today", async () => {
    const bot = createBot(deps, FAKE_BOT_INFO);
    const sent = repliesFrom(bot);

    await bot.init();
    await bot.handleUpdate(textUpdate("/task buy milk"));
    await bot.handleUpdate(textUpdate("/today"));

    expect(sent[0]).toMatch(/^Task captured/);
    expect(sent[1]).toContain("buy milk");
  });

  it("captures a topic via /topic and surfaces it via /inbox", async () => {
    const bot = createBot(deps, FAKE_BOT_INFO);
    const sent = repliesFrom(bot);

    await bot.init();
    await bot.handleUpdate(textUpdate("/topic spaced repetition"));
    await bot.handleUpdate(textUpdate("/inbox"));

    expect(sent[1]).toContain("spaced repetition");
  });

  it("completes a task end-to-end via /task then /done", async () => {
    const bot = createBot(deps, FAKE_BOT_INFO);
    const sent = repliesFrom(bot);

    await bot.init();
    await bot.handleUpdate(textUpdate("/task write the report"));
    const shortId = sent[0].match(/\[(\w+)\]/)?.[1] ?? "";
    await bot.handleUpdate(textUpdate(`/done ${shortId}`));
    await bot.handleUpdate(textUpdate("/today"));

    expect(sent[1]).toBe("Done: write the report");
    expect(sent[2]).toBe("Nothing due today.");
  });

  it("only ever drafts via /draft, never publishes", async () => {
    const bot = createBot(deps, FAKE_BOT_INFO);
    const sent = repliesFrom(bot);

    await bot.init();
    await bot.handleUpdate(textUpdate("/draft announce the launch"));

    expect(sent[0]).toMatch(/awaiting approval/);
  });

  it("reports pnl and quiz with no data yet", async () => {
    const bot = createBot(deps, FAKE_BOT_INFO);
    const sent = repliesFrom(bot);

    await bot.init();
    await bot.handleUpdate(textUpdate("/pnl"));
    await bot.handleUpdate(textUpdate("/quiz"));

    expect(sent[0]).toBe("All-time realized PnL: +0.00");
    expect(sent[1]).toMatch(/No topics/);
  });

  it("only sees its own workspace's tasks", async () => {
    const otherDeps: BotDeps = { token: "fake-token", db, workspaceId: "ws_other" };
    const botA = createBot(deps, FAKE_BOT_INFO);
    const botB = createBot(otherDeps, FAKE_BOT_INFO);
    const sentA = repliesFrom(botA);
    const sentB = repliesFrom(botB);

    await botA.init();
    await botB.init();
    await botA.handleUpdate(textUpdate("/task workspace a's task"));
    await botB.handleUpdate(textUpdate("/today"));

    expect(sentB[0]).toBe("Nothing due today.");
  });
});
