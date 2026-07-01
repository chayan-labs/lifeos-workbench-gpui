# Manual setup required

Everything in this repo that I can build, configure, or wire myself gets done
without asking.
This file is only for steps that genuinely require something only you can do:
an account only you can create, a credential only you can issue, a real-world
choice (domain name, billing plan) only you can make, or a physical machine
action (granting a permission dialog, plugging in a device).

Each entry says which issue it blocks and exactly what to do. Nothing in this
codebase currently depends on production hosting - the dev server is the only
target until you decide to deploy.

## Pending

### #47 - deploy self-hosted Nango + register the first OAuth apps

The code (`infra/nango/docker-compose.yml`, `services/lifeos-api/src/nango.rs`,
`/api/connections/*`) is built and tested against a mock. Bringing up a real
Nango instance and connecting a real account needs you:

1. **Generate secrets** (from `infra/nango/`):
   ```sh
   cp .env.example .env
   openssl rand -base64 32   # -> NANGO_ENCRYPTION_KEY (back this up outside git - immutable once real connections exist)
   openssl rand -hex 32      # -> NANGO_SECRET_KEY_DEV (and _PROD if you want a separate prod secret)
   ```
   Pick a Postgres password and dashboard username/password while you're in there.

2. **Bring it up**: `docker compose up -d` from `infra/nango/`. Dashboard at
   `http://localhost:3003` (basic-auth with the credentials you just set).

3. **Register a GitHub OAuth app** (developer settings -> OAuth Apps -> New):
   - Homepage URL: `http://localhost:3003`
   - Authorization callback URL: `http://localhost:3003/oauth/callback`
   - Copy the client id/secret into an "github" integration in the Nango dashboard.

4. **Register a Google Cloud OAuth client** (APIs & Services -> Credentials ->
   Create OAuth client ID, type "Web application" - covers Gmail+Calendar+Drive,
   issues #48/56/57/58):
   - Authorized redirect URI: `http://localhost:3003/oauth/callback`
   - Enable the Gmail, Calendar, and Drive APIs on the project.
   - Copy the client id/secret into a "google" integration in the Nango dashboard.

5. **Set `NANGO_SERVER_URL` and `NANGO_SECRET_KEY_DEV`** in lifeos-api's own
   env (not `infra/nango/.env` - the API process reads these directly) so
   `build_state()` wires the real client instead of leaving `/api/connections`
   at NotImplemented.

6. **Smoke test**: `POST /api/connections/session` with `{"provider": "github"}`,
   open the returned session in Nango's Connect UI (port 3009), complete the
   OAuth flow, then `POST /api/connections/complete` with the `connectionId`
   it gives you. Confirm the token never appears in `lifeos-api`'s logs or in
   the `/api/connections` response body - only `nango_connection_id`/
   `status`/`provider` should be visible.

This unblocks #48-55 (the rest of the integrations phase), which reuse this
same Nango deployment and only need their own provider app registered.

### #48 - Google app (Gmail + Calendar + Drive scopes)

Covered by step 4 above. Scopes to request on the OAuth consent screen:
`gmail.readonly` + `gmail.modify` (send stays gated at the API layer
regardless), `calendar` (read+write), `drive.readonly` + `drive.file`
(never blanket `drive` - `drive.file` only sees what the app itself creates).
No new code needed: `POST /api/connections/session {"provider": "google"}`
already works once the "google" integration exists in the Nango dashboard.

### #49 - Notion / Slack / GitHub / Figma apps

No new code needed - each is `POST /api/connections/session
{"provider": "<key>"}` once its integration is added in the Nango dashboard
(GitHub's OAuth app is already covered by #47 step 3). For each:

- **Notion**: notion.so/my-integrations -> New integration, capabilities
  "Read content" (+ "Update content" for the two-way sync #59 needs later).
  Redirect URI: `http://localhost:3003/oauth/callback`.
- **Slack**: api.slack.com/apps -> Create New App -> From scratch. OAuth
  scopes: `channels:read`, `channels:history`, `chat:write` (posting stays
  gated at the API layer). Redirect URL: `http://localhost:3003/oauth/callback`.
- **Figma**: figma.com/developers/apps -> Create new app. Callback:
  `http://localhost:3003/oauth/callback`. (Bulk of Figma access is via
  mcp-figma at runtime - this Nango connection is only for file *metadata*.)

### #50 - Meta (Instagram) / X / Reddit apps

No new code needed for Instagram/X/Reddit - same pattern as #49. WhatsApp
is not a Meta Cloud API connector - it's a self-hosted native connector
(GOWA/whatsmeow), tracked separately as #52.

- **Meta app** (developers.facebook.com/apps -> Create App -> type
  "Business"): add the Instagram Graph API product, request
  `instagram_basic` + `instagram_content_publish` (publish stays gated).
  Redirect URI: `http://localhost:3003/oauth/callback`.
- **X/Twitter app** (developer.x.com -> Projects & Apps -> Create App):
  OAuth 2.0, scopes `tweet.read` + `tweet.write` + `users.read` (write
  stays gated). Callback: `http://localhost:3003/oauth/callback`.
- **Reddit app** (reddit.com/prefs/apps -> create app, type "web app"):
  redirect URI `http://localhost:3003/oauth/callback`, scopes `read` +
  `submit` (submit stays gated).

### #51 - Zerodha Kite Connect app (read-only positions)

Kite doesn't fit Nango's OAuth model (daily request-token, not a refresh
token), so it's a native custom connector
(`services/lifeos-api/src/kite.rs`, `/api/connections/kite/*`,
`/api/broker/positions`). The code is built and tested against a mock -
trading stays read-only by construction (no place/modify/cancel/GTT route
exists anywhere on the router; `broker-guard` is the separate hook-layer
belt-and-suspenders). Bringing up a real connection needs you:

1. **Register a Kite Connect app** at developers.kite.trade -> Create new
   app. This is a *paid* Zerodha developer subscription (unlike every other
   integration in this doc) - check current pricing before registering.
   Redirect URL: point it at wherever the frontend will read the
   `request_token` query param and POST it to `/api/connections/kite/complete`
   (a local dev URL is fine to start, e.g. `http://localhost:5173/kite/callback`).

2. **Generate the shared secret-encryption key** (this key also covers #52's
   WhatsApp connector - generate it once):
   ```sh
   openssl rand -base64 32   # -> LIFEOS_SECRET_ENCRYPTION_KEY (back this up outside git - encrypted connections.secret_enc rows become unreadable if it's lost)
   ```

3. **Set lifeos-api's env**: `KITE_API_KEY`, `KITE_API_SECRET` (from the Kite
   Connect app), and `LIFEOS_SECRET_ENCRYPTION_KEY` from step 2. Until all
   three are set, `/api/connections/kite/*` and `/api/broker/positions`
   return 501.

4. **Daily login** (Kite's access_token expires every day around 6am IST -
   there is no way around re-logging in daily, by Kite's own design): visit
   `GET /api/connections/kite/login-url`, open it, log in, and the redirect
   will carry a `request_token` - POST that to `/api/connections/kite/complete`.

5. **Smoke test**: `GET /api/broker/positions` returns your real positions.
   Confirm the access token never appears in `lifeos-api`'s logs or in any
   `/api/connections`/`/api/broker/positions` response body - only
   `account_handle`/`status`/`provider` should be visible for the connection,
   and `positions` returns Kite's data with no token field.

### #52 - WhatsApp via self-hosted GOWA (QR-pair, no Meta app)

WhatsApp is a native custom connector too, but unlike Kite it needs no paid
developer account and no app registration at all - `infra/gowa/` runs
[go-whatsapp-web-multidevice](https://github.com/aldinokemal/go-whatsapp-web-multidevice)
(GOWA, MIT), a REST wrapper around `whatsmeow` that talks to WhatsApp's own
protocol directly, the same way WhatsApp Web does. GOWA has a real
multi-tenant device API - each workspace gets its own GOWA "device" keyed by
`device_id = workspace_id` - and a single server-wide webhook, so there's no
per-workspace secret to mint or encrypt this time (`connections.secret_enc`
stays `NULL` for WhatsApp rows; GOWA auth is one shared Basic Auth
credential lifeos-api alone holds). Inbound messages are captured
(`entities` with `module='integrations', type='whatsapp_message'`); sending
is gated - `POST /api/whatsapp/send` only creates a draft entity, it never
calls GOWA (the approve+execute leg ships with the Bot/executor phase, not
here).

**:warning: Read this before pairing a number:** GOWA's own README warns
this drives a real WhatsApp account over its consumer protocol - using it
for spam, bulk sends, or anything automated at scale risks WhatsApp banning
the number. Fine for personal use (which is all this connects to today);
don't repurpose it for bulk outbound without understanding that risk.

1. **Bring up GOWA** (from `infra/gowa/`):
   ```sh
   cp .env.example .env
   openssl rand -hex 16      # -> the password half of GOWA_BASIC_AUTH (format: user:pass)
   openssl rand -base64 32   # -> GOWA_WEBHOOK_SECRET (must match lifeos-api's GOWA_WEBHOOK_SECRET exactly)
   docker compose up -d
   ```
   Check the pinned image tag in `infra/gowa/docker-compose.yml` still
   exists on Docker Hub before this step - bump it if the upstream project
   has moved on.

2. **Set lifeos-api's env**: `GOWA_BASE_URL` (e.g. `http://127.0.0.1:8082`),
   `GOWA_BASIC_AUTH` (same `user:pass` as step 1), `GOWA_WEBHOOK_SECRET`
   (same value as above). Until all three are set,
   `/api/connections/whatsapp/*` and `/api/webhooks/whatsapp` return 501.

3. **Pair a device**: `POST /api/connections/whatsapp/session`, then
   `GET /api/connections/whatsapp/qr` and open the returned `qr_link` (a
   GOWA-served image URL, local network only) - scan it with the WhatsApp
   mobile app (Linked Devices -> Link a Device) on the number you want
   connected. Poll `GET /api/connections/whatsapp/status` until
   `connected: true`.

4. **Wire the inbound webhook** (optional, needed for message capture): set
   `WHATSAPP_WEBHOOK` in `infra/gowa/.env` to a publicly-reachable URL for
   this API's `/api/webhooks/whatsapp` (a tunnel like ngrok/Cloudflare
   Tunnel during dev, since GOWA runs outside this Mac's localhost from
   WhatsApp's point of view) *before* bringing GOWA up - unlike the earlier
   wuzapi design this is a static, server-wide setting, not something
   lifeos-api registers dynamically. Leave it blank to skip inbound capture
   entirely; pairing and the gated send-draft still work without it.

5. **Smoke test**: send yourself a WhatsApp message from another device -
   an `entities` row (`module='integrations', type='whatsapp_message'`)
   should appear. Confirm the GOWA Basic Auth credential never appears in
   any `/api/connections` response body.

### #53 - Gmail / Calendar / Drive / Notion / Slack proxy tools

No new code needed beyond #47/#48/#49's Nango setup - `/api/gmail/list`,
`/api/calendar/list`, `/api/drive/list`, `/api/notion/list`, and
`/api/slack/list` all proxy through the same self-hosted Nango deployment.
Complete a `POST /api/connections/session {"provider": "google-mail"}` (and
`google-calendar`/`google-drive`/`notion`/`slack`) connect flow for each
provider you want reachable - `list` 404s until its connection is active.
Gated writes (`gmail/send`, `calendar/create`, `drive/upload`,
`notion/create`, `slack/post`) work with no connection at all, since they
never call the provider.

### #54 - Browser actuator (`external/browser-use`)

The submodule at `external/browser-use` is already checked out (pinned
commit, `.gitmodules`). Bringing up a real browser session needs you:

1. **Install the Python side** (from `external/browser-use/`):
   ```sh
   uv sync --extra core   # or: pip install -e ".[core]"
   uv run python -m playwright install chromium   # or: python3 -m playwright install chromium
   ```

2. **Set an LLM key for the browser agent's own reasoning** (separate from
   any Claude Code key - browser-use runs its own agent loop):
   ```sh
   export ANTHROPIC_API_KEY=...   # or GOOGLE_API_KEY / OPENAI_API_KEY, matching scripts/browser_actuator.py's _llm()
   ```

3. **Set lifeos-api's env**: `BROWSER_ACTUATOR_SCRIPT` (path to
   `services/lifeos-api/scripts/browser_actuator.py`) and
   `LIFEOS_SECRET_ENCRYPTION_KEY` (reuse the same key generated for #51/#52
   if already set - it covers browser sessions too). Until both are set,
   `/api/browser/scrape` and `/api/connections/browser/session` return 501
   (`/api/browser/act` always works - it only ever drafts, it needs no
   actuator configured at all).

4. **Smoke test the free path**:
   `POST /api/browser/scrape {"url": "https://example.com", "task": "read the page title"}`
   should run headless and return a result - no login needed.

5. **Capture a session for a site that needs login** (optional):
   `POST /api/connections/browser/session {"site": "example.com"}` opens a
   real, visible Chromium window - log in yourself when it appears, then let
   the agent finish. The captured session never appears in the response;
   confirm no `secret_enc` field and no raw cookie value shows up in
   `/api/connections`.

### #63 - Deploy the Telegram bot Worker (`worker/`)

The grammY bot + Cloudflare Worker scaffold (`worker/src/bot.ts`, `worker/src/index.ts`,
`worker/wrangler.toml`) is committed and passes `npm test`/`npm run typecheck`/
`wrangler deploy --dry-run` locally - it only needs your Cloudflare account and a real
Telegram bot token to actually go live:

1. **Create the Telegram bot**: message
   [@BotFather](https://t.me/BotFather) → `/newbot` → note the token it gives you.

2. **Authenticate wrangler** (from `worker/`):
   ```sh
   npx wrangler login   # or export CLOUDFLARE_API_TOKEN
   ```

3. **Set the bot token as a Worker secret** (never committed - `wrangler.toml` only
   documents the name):
   ```sh
   npx wrangler secret put BOT_TOKEN
   ```

4. **Deploy**:
   ```sh
   npm run deploy
   ```
   note the `https://<subdomain>.workers.dev` URL wrangler prints.

5. **Register the webhook** so Telegram forwards updates to the deployed Worker:
   ```sh
   curl "https://api.telegram.org/bot$BOT_TOKEN/setWebhook?url=https://<subdomain>.workers.dev/telegram"
   ```

6. **Smoke test**: message the bot `/start` and `/health` from Telegram - it should reply
   "Life OS bot is online." and "ok" respectively. This is the only part of #63's acceptance
   criteria ("bot responds to a message on the deployed Worker") that needs a live deploy;
   everything else is covered by `worker/test/*.test.ts`.

### #64 - Turso + Haiku secrets for the bot's DB/LLM bindings

`worker/src/db.ts` (workspace-scoped Drizzle queries over `@lifeos/db/client/worker`) and
`worker/src/llm.ts` (Haiku via `@anthropic-ai/sdk`) are committed and covered by
`worker/test/entities.test.ts`/`llm.test.ts` (an in-memory libSQL DB and a stubbed
`fetch`, no live services touched). Not yet wired into any bot command - that's #65.
Getting the real bindings live (once #65 lands) needs three more Worker secrets,
alongside #63's `BOT_TOKEN`:

```sh
npx wrangler secret put TURSO_URL          # e.g. libsql://<db>-<org>.turso.io - same DB the Mac API writes to
npx wrangler secret put TURSO_TOKEN        # Turso auth token
npx wrangler secret put ANTHROPIC_API_KEY  # Haiku key for the bot's own reasoning
```

`WORKSPACE_ID` is a non-secret var (`wrangler.toml`'s `[vars]`) and defaults to
`"default-personal-workspace"` if left unset - the same default
`services/lifeos-api/src/config.rs::DEFAULT_WORKSPACE` uses, so the bot and the SPA/API
read and write the same rows until real multi-user auth exists (phase 7).

**:warning: Read this before running any `act` you approve:** the browser
actuator can do anything a logged-in you can on the sites it has a captured
session for - it is `docs/SECURITY.md` §4's most powerful and most dangerous
integration. Only approve drafted `browser.act` entities whose task string
you've actually read.

### #71 - enable the daily digest (`DIGEST_CHAT_ID`)

`worker/src/digest.ts` and the `scheduled` handler in `worker/src/index.ts` are committed
and covered by `worker/test/digest.test.ts`/`index.test.ts` (a local DB and an early-return
check, no live Telegram send). The cron trigger itself (`wrangler.toml`'s
`[triggers] crons = ["0 8 * * *"]`) is already deployed with every `wrangler deploy` - it
just no-ops until you set the chat to send to:

```sh
# message the bot once first, then read the chat id off the update, or ask
# @userinfobot - it's the same chat id Telegram already uses for every reply.
npx wrangler secret put DIGEST_CHAT_ID   # not actually secret, but simplest as a secret put here
```

(`DIGEST_CHAT_ID` is declared as a `[vars]` entry in `wrangler.toml` for local dev/`.dev.vars`;
either a `wrangler secret put` or an uncommented `[vars]` line works in production - it isn't
sensitive, it's just where the digest goes.) Edit the cron expression in `wrangler.toml` and
redeploy to change the send time.

### #75 - one-time Playwright browser download for the render-smoke validator

`server/validators/render.js` (docs/SELF-EXTENSION.md §4) drives headless Chromium via
`playwright`, which ships as an npm package but not with a browser binary - download it once:

```sh
cd server && npx playwright install chromium
```

Also requires `cargo build --bin lifeos-api` (services/) and `npm install` (frontend/) to
have been run at least once, since the validator boots both as real child processes.

### #78 - `lifeos-drain` env for the real offline build (`TELEGRAM_BOT_TOKEN`, `LIFEOS_SERVER_DIR`)

`services/lifeos-drain` now actually builds bot-queued `/addmodule` requests
(`docs/SELF-EXTENSION.md` §1b) and notifies the requester's Telegram chat on completion. Two
env vars, both read in `services/lifeos-drain/src/main.rs`:

```sh
# Same value as the Worker's BOT_TOKEN secret (§63/§71 above) - it's the same bot,
# this is just how the Mac-side drain calls the Telegram API directly instead of
# routing through Cloudflare. Without this set, builds still complete/fail
# correctly, they just aren't announced back to the phone.
export TELEGRAM_BOT_TOKEN="<same token as worker's BOT_TOKEN>"

# Directory scaffold.js lives in, relative to wherever the lifeos-drain binary
# is launched from (a compiled binary's cwd isn't guaranteed to be the repo
# root - set this explicitly in the launchd plist). Defaults to "server".
export LIFEOS_SERVER_DIR="/path/to/life-os/server"
```

A real build additionally needs everything #72 already required (`ANTHROPIC_API_KEY`, git
worktree support in the checkout `lifeos-drain` runs against) and #75's `npx playwright
install chromium` (the render-smoke validator `scaffoldModule` calls). One true end-to-end
check once all of the above is in place: send `/addmodule <something>` to the real bot, run
`lifeos-drain` locally, confirm a real commit lands on `main` under `modules/<id>/` and a real
"✅ live" message arrives in the chat that sent the request.
`server/scripts/renderSmokeLive.js` is a manual smoke check once all three are done:

```sh
node server/scripts/renderSmokeLive.js   # expects {"valid":true,"errors":[]}
```

### #83 - `lifeos-vcs` R2 blob mirror (`R2_BUCKET`, `R2_ENDPOINT`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`)

`services/lifeos-vcs::BlobMirror::from_r2_env()` (docs/VERSIONING.md §2.1/§5) mirrors CAS
objects to Cloudflare R2 out-of-band from the libSQL replica. Requires an R2 bucket and an
R2 API token (Cloudflare dashboard → R2 → Manage R2 API Tokens → create a token with
Object Read & Write on the bucket):

```sh
export R2_BUCKET="lifeos-blobs"
export R2_ENDPOINT="https://<account_id>.r2.cloudflarestorage.com"
export R2_ACCESS_KEY_ID="<r2 access key id>"
export R2_SECRET_ACCESS_KEY="<r2 secret access key>"
```

The automated test suite exercises the same `mirror_object`/`pull_object`/`pull_on_demand`
code paths against a local-filesystem `object_store` backend standing in for R2 (no network,
no credentials needed for `cargo test`). One true end-to-end check once the bucket/token
above are set: mirror a blob, delete it from the local CAS directory, confirm
`pull_on_demand` fetches it back from the real bucket and the BLAKE3 hash still matches.
