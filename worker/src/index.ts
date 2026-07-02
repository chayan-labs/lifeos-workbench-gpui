// Cloudflare Worker entrypoint - issues #63-65 (docs/ARCHITECTURE.md §3.1).
// Routes Telegram's webhook POSTs into grammY via the native Workers
// adapter (`webhookCallback(bot, "cloudflare-mod")`); everything else is a
// bare liveness check for `wrangler deploy` smoke-testing.
import { webhookCallback } from "grammy";
import { createBot } from "./bot.js";
import { createDb, resolveWorkspaceId } from "./db.js";
import { buildDigest } from "./digest.js";

export interface Env {
  BOT_TOKEN: string;
  // DB + Haiku bindings (issues #64/#65).
  TURSO_URL: string;
  TURSO_TOKEN: string;
  // OPTIONAL - the bot's own light reasoning lane. Unset is fine: the bot
  // still does full DB CRUD/recall, and heavy or AI work is enqueued to the
  // Mac harness (jobs), which runs keyless through local agent CLIs
  // (services/lifeos-agents). Cloudflare can't exec CLIs, so this is the
  // only key left in the system, and it is opt-in.
  ANTHROPIC_API_KEY?: string;
  WORKSPACE_ID?: string;
  // issue #71 - where the scheduled digest is sent; unset = no digest
  // (manual-setup-gated, same as every other deploy-time value in this
  // file, see docs/MANUAL-SETUP.md). Get this by messaging the bot once and
  // reading the chat id off the update, or from @userinfobot.
  DIGEST_CHAT_ID?: string;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (url.pathname === "/") {
      return new Response("ok", { status: 200 });
    }

    if (url.pathname === "/telegram" && request.method === "POST") {
      const db = createDb(env);
      const workspaceId = resolveWorkspaceId(env);
      const bot = createBot({ token: env.BOT_TOKEN, db, workspaceId });
      const handleUpdate = webhookCallback(bot, "cloudflare-mod");
      return handleUpdate(request);
    }

    return new Response("not found", { status: 404 });
  },

  // Cloudflare Cron Trigger (wrangler.toml's `[triggers] crons`), issue #71.
  // No-ops when DIGEST_CHAT_ID isn't set - real send is verified live
  // post-deployment, same as `/telegram` (worker/test/index.test.ts).
  async scheduled(_event: ScheduledEvent, env: Env): Promise<void> {
    if (!env.DIGEST_CHAT_ID) return;

    const db = createDb(env);
    const workspaceId = resolveWorkspaceId(env);
    const digest = await buildDigest(db, workspaceId, Math.floor(Date.now() / 1000));
    const bot = createBot({ token: env.BOT_TOKEN, db, workspaceId });
    await bot.api.sendMessage(Number(env.DIGEST_CHAT_ID), digest);
  },
};
