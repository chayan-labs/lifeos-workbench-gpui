// grammY bot definition - issues #63-67.
import { Bot, InlineKeyboard } from "grammy";
import type { UserFromGetMe } from "grammy/types";
import type { WorkerDb } from "@lifeos/db/client/worker";
import { approveEntity, denyEntity, listPendingApprovals } from "./approvals.js";
import {
  captureDraft,
  captureTask,
  captureTopic,
  formatApprovalResult,
  formatPendingApproval,
  inbox,
  ingest,
  markDone,
  pnl,
  quiz,
  recall,
  requestModule,
  today,
} from "./commands.js";

export function healthMessage(): string {
  return "ok";
}

export interface BotDeps {
  token: string;
  db: WorkerDb;
  workspaceId: string;
}

const APPROVE_PREFIX = "approve:";
const DENY_PREFIX = "deny:";

function approvalKeyboard(entityId: string): InlineKeyboard {
  return new InlineKeyboard().text("Approve", `${APPROVE_PREFIX}${entityId}`).text("Deny", `${DENY_PREFIX}${entityId}`);
}

// `botInfo` lets tests construct a Bot without a network call to Telegram's
// getMe (grammY's documented pattern for testing bots offline).
export function createBot(deps: BotDeps, botInfo?: UserFromGetMe): Bot {
  const bot = new Bot(deps.token, botInfo ? { botInfo } : undefined);
  const { db, workspaceId } = deps;

  bot.command("start", async (ctx) => {
    await ctx.reply("Life OS bot is online.");
  });

  bot.command("health", async (ctx) => {
    await ctx.reply(healthMessage());
  });

  bot.command("task", async (ctx) => {
    await ctx.reply(await captureTask(db, workspaceId, ctx.match));
  });

  bot.command("topic", async (ctx) => {
    await ctx.reply(await captureTopic(db, workspaceId, ctx.match));
  });

  bot.command("draft", async (ctx) => {
    const { reply, entity } = await captureDraft(db, workspaceId, ctx.match);
    await ctx.reply(reply, entity ? { reply_markup: approvalKeyboard(entity.id) } : undefined);
  });

  bot.command("done", async (ctx) => {
    await ctx.reply(await markDone(db, workspaceId, ctx.match));
  });

  bot.command("today", async (ctx) => {
    await ctx.reply(await today(db, workspaceId, Math.floor(Date.now() / 1000)));
  });

  bot.command("inbox", async (ctx) => {
    await ctx.reply(await inbox(db, workspaceId));
  });

  bot.command("pnl", async (ctx) => {
    await ctx.reply(await pnl(db, workspaceId));
  });

  bot.command("quiz", async (ctx) => {
    await ctx.reply(await quiz(db, workspaceId));
  });

  // issue #69: lexical fallback recall - see recall.ts for why this isn't
  // the full FTS5+memvec RRF hybrid (that lives on the Mac-only derived DB).
  bot.command("recall", async (ctx) => {
    await ctx.reply(await recall(db, workspaceId, ctx.match));
  });

  // issue #67: heavy/Mac-only work - the bot only ever enqueues, never
  // builds or executes anything itself.
  bot.command("addmodule", async (ctx) => {
    await ctx.reply(await requestModule(db, workspaceId, ctx.match));
  });

  bot.command("ingest", async (ctx) => {
    await ctx.reply(await ingest(db, workspaceId, ctx.match));
  });

  // issue #66: everything awaiting a tap, one message per draft (Telegram
  // has no multi-item-keyboard primitive), each with its own approve/deny
  // buttons.
  bot.command("pending", async (ctx) => {
    const pending = await listPendingApprovals(db, workspaceId);
    if (pending.length === 0) {
      await ctx.reply("Nothing pending approval.");
      return;
    }
    for (const entity of pending) {
      await ctx.reply(formatPendingApproval(entity), { reply_markup: approvalKeyboard(entity.id) });
    }
  });

  bot.on("callback_query:data", async (ctx) => {
    const data = ctx.callbackQuery.data;
    const isApprove = data.startsWith(APPROVE_PREFIX);
    const isDeny = data.startsWith(DENY_PREFIX);
    if (!isApprove && !isDeny) {
      await ctx.answerCallbackQuery();
      return;
    }

    const entityId = data.slice(isApprove ? APPROVE_PREFIX.length : DENY_PREFIX.length);
    const result = isApprove ? await approveEntity(db, workspaceId, entityId) : await denyEntity(db, workspaceId, entityId);
    const text = formatApprovalResult(result);

    await ctx.answerCallbackQuery({ text });
    await ctx.editMessageText(text).catch(() => {
      // Original message may already be edited/gone (e.g. a second tap
      // racing the first) - the answerCallbackQuery toast above already
      // told the user the outcome either way.
    });
  });

  return bot;
}
