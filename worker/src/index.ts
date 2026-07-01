// Cloudflare Worker entrypoint - issue #63 (docs/ARCHITECTURE.md §3.1).
// Routes Telegram's webhook POSTs into grammY via the native Workers
// adapter (`webhookCallback(bot, "cloudflare-mod")`); everything else is a
// bare liveness check for `wrangler deploy` smoke-testing.
import { webhookCallback } from "grammy";
import { createBot } from "./bot.js";

export interface Env {
  BOT_TOKEN: string;
  // DB + Haiku bindings (issue #64) - unused by fetch() until #65 wires
  // capture/query commands, but declared here so wrangler.toml's secrets
  // line up with what db.ts/llm.ts expect.
  TURSO_URL: string;
  TURSO_TOKEN: string;
  ANTHROPIC_API_KEY: string;
  WORKSPACE_ID?: string;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (url.pathname === "/") {
      return new Response("ok", { status: 200 });
    }

    if (url.pathname === "/telegram" && request.method === "POST") {
      const bot = createBot(env.BOT_TOKEN);
      const handleUpdate = webhookCallback(bot, "cloudflare-mod");
      return handleUpdate(request);
    }

    return new Response("not found", { status: 404 });
  },
};
