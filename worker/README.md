# @lifeos/worker

Cloudflare Worker hosting the Telegram bot (grammY, `webhookCallback(bot, "cloudflare-mod")`)
and, later, OAuth callbacks. See `docs/ARCHITECTURE.md` §3.1 and `docs/BUILD-PLAN.md` phase 4.

- Issue #63: bot scaffold, `/start` and `/health` commands.
- Issue #64: `src/db.ts` (workspace-scoped `@lifeos/db/client/worker` binding) and
  `src/llm.ts` (Haiku via `@anthropic-ai/sdk`).
- Issue #65: capture/query commands (`src/commands.ts`), wired into the bot:
  - `/task <text>`, `/topic <text>` - capture into `tasks`/`learning`.
  - `/done <id-suffix>` - completes a task by the tail of its id (shown in `/task`'s and
    `/today`'s replies).
  - `/today` - open tasks due today or undated.
  - `/inbox` - captures with no status yet (e.g. a fresh `/topic`).
  - `/pnl` - realized PnL summed from `trade.closed` events (read-only, never a broker call).
  - `/quiz` - spaced-repetition-style prompt, naive (oldest-untouched topic).
  - `/draft <text>` - creates a `pending_approval` entity; never publishes anything itself.

Gated approve/deny keyboards and the heavy-job enqueue path land in #66-67.

Every DB query in `src/entities.ts` filters by `workspace_id`, resolved server-side from
`env.WORKSPACE_ID` (never from Telegram input) via `resolveWorkspaceId()` in `src/db.ts`.
Import query builders (`and`/`eq`/`sql`/...) from `@lifeos/db/query`, not `"drizzle-orm"`
directly - a second independently-installed copy of `drizzle-orm` produces branded types
that don't structurally match `@lifeos/db`'s schema, breaking every query at the type
level.

## Develop

```
npm install
npm test         # vitest, no network - grammY's offline pattern + an in-memory libSQL DB
npm run typecheck
npm run dev       # wrangler dev, needs .dev.vars (see docs/MANUAL-SETUP.md #63/#64)
```

## Deploy (manual, one-time - see `docs/MANUAL-SETUP.md`)

```
wrangler login                       # or set CLOUDFLARE_API_TOKEN
wrangler secret put BOT_TOKEN        # Telegram bot token from @BotFather
wrangler secret put TURSO_URL        # issue #64 - same DB the Mac API writes to
wrangler secret put TURSO_TOKEN
wrangler secret put ANTHROPIC_API_KEY
npm run deploy
# then register the webhook so Telegram forwards updates to the deployed Worker
curl "https://api.telegram.org/bot$BOT_TOKEN/setWebhook?url=https://<worker-subdomain>.workers.dev/telegram"
```
