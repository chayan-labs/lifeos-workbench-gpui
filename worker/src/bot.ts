// grammY bot definition - issues #63-65. Gated approve/deny keyboards and
// the heavy-job enqueue path land in #66-67.
import { Bot } from "grammy";
import type { UserFromGetMe } from "grammy/types";
import type { WorkerDb } from "@lifeos/db/client/worker";
import { captureDraft, captureTask, captureTopic, inbox, markDone, pnl, quiz, today } from "./commands.js";

export function healthMessage(): string {
  return "ok";
}

export interface BotDeps {
  token: string;
  db: WorkerDb;
  workspaceId: string;
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
    await ctx.reply(await captureDraft(db, workspaceId, ctx.match));
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

  return bot;
}
