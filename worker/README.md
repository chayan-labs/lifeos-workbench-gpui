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
- Issue #66: gated approve/deny (`src/approvals.ts`, `src/jobs.ts`, docs/SECURITY.md §2).
  `/draft`'s confirmation message carries an inline Approve/Deny keyboard; `/pending` lists
  every `pending_approval` entity in the workspace (not just bot-originated drafts) with the
  same buttons. Tapping Approve transitions the entity to `approved`, records
  `events('${type}.approved')`, and enqueues `jobs(kind='execute_approval')` for the Mac to
  drain - the Worker never calls a provider directly. Tapping Deny records
  `events('${type}.rejected')` and enqueues nothing. A second tap on an already-resolved
  draft is a no-op, not a crash.
- Issue #67: heavy-job enqueue (`src/moduleRequests.ts`, docs/ARCHITECTURE.md §5).
  `/addmodule <prompt>` writes a `module_requests` row (`status='queued'`); `/ingest <text>`
  writes a generic `jobs(kind='ingest')` row via the same `enqueueJob` #66's
  `execute_approval` jobs use. Both reply "queued" and touch no filesystem - there isn't one
  on a Worker.

Real dispatch - `services/lifeos-drain` actually claiming and running `execute_approval`
(#66) or `ingest` (#67) jobs, or draining `module_requests` into a real scaffold.js build -
is not built yet; `dispatch()`'s other job kinds (`pipeline`/`module_build`/`eval`/
`reconcile`) are stubs too. The bot side of both queues is done and tested; the drain side
is a separate, not-yet-scoped piece of work.
- Issue #69: `/recall <query>` (`src/recall.ts`) - lexical fallback recall, workspace-scoped,
  citing the matched entity by short id. NOT the real hybrid: `services/lifeos-api/src/
  routes/search.rs` already implements FTS5+memvec RRF fusion over `lifeos-derived.db`, but
  that DB is intentionally un-synced/Mac-local (docs/DATA-MODEL.md §5) and memvec.py is a
  Python subprocess - neither is reachable from a Cloudflare Worker (no filesystem, and the
  Mac API only binds 127.0.0.1). This command trades recall quality for laptop-off
  availability: a case-insensitive `LIKE` over `title`/`attrs` in the canonical Turso DB.
- Issue #70: audit logging - a `bot.use` middleware in `src/bot.ts` records an
  `events(type='bot.command', actor='bot', attrs={text})` row for every incoming `/command`
  before it runs (append-only, never an UPDATE/DELETE), so bot activity - including plain
  reads like `/today`/`/pnl`, not just #66's approve/deny transitions - is fully
  reconstructable from `events`. Non-command text (no `/` prefix) is not logged, since it's
  never routed to a handler either.
- Issue #71: daily digest - `src/digest.ts::buildDigest` rolls up `/today`, `/inbox`
  (uncategorized/blocked), `/pnl`, and pending approvals into one message, reusing those
  already-tested commands rather than re-querying. `index.ts` exports a `scheduled` handler
  (Cloudflare Cron Trigger, `wrangler.toml`'s `[triggers] crons`) that builds and sends it -
  a no-op unless `DIGEST_CHAT_ID` is set (manual, see `docs/MANUAL-SETUP.md`). PWA push
  mirroring the same digest is Phase 7, not this issue.

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
