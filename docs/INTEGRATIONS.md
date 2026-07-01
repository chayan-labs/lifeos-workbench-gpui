# Integrations - the owned-credential model

> **The defining constraint:** Life OS must NOT depend on the claude.ai MCP connectors (Gmail/Calendar/Drive/Notion/Slack).
> Those live inside a claude.ai account that is **not ours** (a lead account we only have Claude Code access to).
> Building on them dies the moment access changes, and they are not SaaS-ready (someone else's OAuth, quota, and tokens).
> **Every third-party integration therefore uses developer apps WE own, via self-hosted Nango + a browser actuator.**

This is not only safer - it is the exact model multi-tenant SaaS requires, so we get the SaaS seam for free.

---

## 1. The model in one picture

```
  Agent / harness                Local lifeos API (Rust)            Provider
  holds only a       ──tool──►   resolves connectionId    ──proxy──►  Gmail / Notion /
  connectionId handle            via Nango, injects token             Slack / X / …
  (NEVER a secret)               server-side                          (token never seen
                                                                       by the agent)
```

- **Nango (self-hosted)** is the OAuth vault: it runs the OAuth dance, stores tokens **encrypted at rest**, **auto-refreshes**, and exposes a **Proxy** (`nango.proxy({connectionId, endpoint})`) that injects the live token server-side and returns only the API response.
- The agent/harness only ever holds a `connectionId` (a row in `connections`), never a token. This is precisely the "API injects tokens at call time, secrets never in agent context" requirement - already implemented by Nango.
- **Reads are free; writes/publishes are human-gated.** Nango is transport, not policy - the gate stays in the local API (route reads straight through the proxy; route writes into the draft → Telegram-approve → execute queue before the proxy fires).

---

## 2. Provider matrix

| Provider | Mechanism | Owned credential | Read | Write |
|---|---|---|---|---|
| Gmail | Nango `google-mail` | your Google Cloud OAuth app | free | 🔒 send |
| Google Calendar | Nango `google-calendar` | same Google app | free | 🔒 create/move |
| Google Drive | Nango `google-drive` | same Google app | free | 🔒 upload/share |
| Notion | Nango `notion` | your Notion integration | free | two-way sync |
| Slack | Nango `slack` (or native bot token) | your Slack app | free | 🔒 post |
| Instagram | Nango | your Meta app | free | 🔒 publish |
| X / Twitter | Nango | your X app | free | 🔒 publish |
| Reddit | Nango | your Reddit app | free | 🔒 post |
| WhatsApp | **native**, self-hosted [go-whatsapp-web-multidevice](https://github.com/aldinokemal/go-whatsapp-web-multidevice) (GOWA, `infra/gowa/`) - no Meta app, QR-pair like WhatsApp Web | your WhatsApp number (device-paired) | free | 🔒 send |
| Zerodha Kite | **native** custom connector (daily request-token, read-scoped) | your Kite app | free (read-only) | **never** (see SECURITY.md) |
| Figma | mcp-figma (on-demand) + Nango for file metadata | your Figma app | free | gated writes |
| Higgsfield | mcp-higgsfield (on-demand, OAuth) | your account | n/a | generate |
| GitHub | Nango `github` (or octocrab in the Rust API) | your GitHub app | free | issues/PRs free; 🔒 merge/release |
| Any no-API service | **browser actuator** (§4) | encrypted browser session | free | 🔒 everything |

**One-time owned setup:** one Google Cloud project (covers Gmail+Calendar+Drive), one Notion integration, one Slack app, one Meta app (IG only), one X app, one Reddit app, one GitHub app, one Figma app, one Kite app, plus self-hosting `infra/gowa/` and pairing a WhatsApp number by QR scan (no Meta app needed for WhatsApp). You fully own all of them.

**Implemented (issue #53):** Gmail, Calendar, Drive, Notion, and Slack each have a thin route pair in `lifeos-api` -
`GET /api/<provider>/list` (free read, proxies straight through Nango) and one gated write
(`POST /api/gmail/send`, `/api/calendar/create`, `/api/drive/upload`, `/api/notion/create`, `/api/slack/post`) that
only ever creates a `pending_approval` draft entity - the handler has no code path to the provider at all (see
`services/lifeos-api/src/integrations.rs`). Reachable from the CLI as `lifeos gmail list`, `lifeos slack post`, etc.
Each `list` 404s until its provider's OAuth app is registered (#48/#49) and a connection is completed - no
per-provider code is missing, only the manual credential step in `docs/MANUAL-SETUP.md`.

**Implemented (issue #55):** the `/integrations` page (`frontend/src/pages/Integrations.jsx`) is a real client of
`GET /api/connections` - no more mock provider list. "Add Connection" calls `POST /api/connections/session`, opens
Nango's self-hosted Connect UI via the official `@nangohq/frontend` SDK (`nango.openConnectUI({baseURL:
VITE_NANGO_CONNECT_URL, ...})`, default `http://localhost:3009`), and on a `connect` event calls
`POST /api/connections/complete` to record the connection. Disconnect calls `DELETE /api/connections/:id`. Native
connectors (Kite, WhatsApp, browser sessions) land in the same `connections` table and list read-only alongside
Nango connections; their own multi-step connect flows (daily login-url, QR scan, headed-browser capture) are
unchanged and out of scope for this generic modal.

---

## 3. Nango details

- **License:** Elastic License 2.0 (source-available). Free to self-host as an internal vault; the only restriction is "don't resell Nango-itself as a managed service" - which Life OS never does. The docker-compose edition covers managed-auth + proxy (the Helm/full self-host path is Enterprise-gated, not needed).
- **Deployment:** runs on the trusted Mac (or a tiny always-on host). Owns OAuth callback URLs. The Cloudflare Worker's only integration role is to forward callbacks if the always-on host is the Worker; it never stores a token.
- **`connections` mapping:** each connected account = one `connections` row carrying `provider`, `account_handle`, `nango_connection_id`. Many accounts per provider = many `connectionId`s. See [DATA-MODEL.md](./DATA-MODEL.md) §3.
- **Custom connectors** (Kite; WhatsApp via self-hosted `infra/gowa/`) that Nango doesn't model cleanly are called by bespoke code in the Rust API - still never in agent context. Kite's daily token is envelope-encrypted in `connections.secret_enc`; WhatsApp has no per-workspace secret to encrypt (GOWA auth is one server-wide Basic Auth credential lifeos-api alone holds).
- **If ELv2 ever conflicts** (it won't for our use): fork Nango (ELv2 permits), or fall back to OpenBao (MPL-2.0) for the vault + hand-rolled OAuth for our ~8 providers (an excellent Rust task). No fully-MIT drop-in equivalent exists - this is the one "might build ourselves" risk. See [LICENSING-REUSE.md](./LICENSING-REUSE.md).

---

## 4. Browser actuator (browser-use)

The universal outward actuator for services with **no API** (or where the API is too limited): the AI drives a real browser to publish, fill dashboards, scrape, book travel, operate any logged-in tool.

- **Source:** `external/browser-use` (Python, vendored as a git submodule of upstream `browser-use/browser-use`). Wrapped as `browser.act` (gated) + `browser.scrape` (free).
- **Where:** Mac-only (trusted), invoked as a subprocess from `lifeos-api` per call - no long-lived MCP process to load/unload.
- **Sessions/cookies:** stored encrypted exactly like `connections`; never in agent context.
- **Gating:** **always human-gated for outward actions** - it can do anything a logged-in you can. Reads/scrapes are free; any state-changing action is draft → approve → execute.
- **Pairing:** Nango (API providers) + browser actuator (everything else) = Life OS can integrate **literally any service**.

**Implemented (issue #54):** `external/browser-use` is vendored as a git submodule (pinned, upstream `browser-use/browser-use`)
and driven from `lifeos-api` via `services/lifeos-api/scripts/browser_actuator.py`
(`src/browser.rs::ProcessBrowserActuator`, same `python3` subprocess pattern as the memvec search lane).
`POST /api/browser/scrape` is free - the Python side runs with every state-changing action (`click`, `input`,
`upload_file`, `send_keys`, `select_dropdown`, `write_file`, `replace_file`, `evaluate`) excluded from the agent's
tool set entirely, so it cannot change external state even if instructed to. `POST /api/browser/act` is gated: it
only ever creates a `pending_approval` draft entity (`services/lifeos-api/src/integrations.rs::draft_action`), which
has no reference to the browser client at all. `POST /api/connections/browser/session` is the one interactive,
Mac-only step - it opens a real headed browser for you to log in yourself, then envelope-encrypts the captured
session (`crypto::encrypt`, the same key as Kite's `secret_encryption_key`) into `connections.secret_enc` with
`provider = 'browser:<site>'` before it ever reaches the DB.

---

## 5. Why MCPs are NOT the connector layer

MCPs in Life OS are reserved strictly for **heavy on-demand capability tools** loaded through mcp-multiplexer - Figma, Higgsfield, game engines (Godot/Unity/Unreal). These are *capability* tools, not *credential-bearing connectors*, and they are unloaded after use for token discipline.

**CRUD and credentialed reads/writes are never an MCP.** They are thin HTTP tools (`~/.claude/bin/lifeos gmail …`) calling the Nango proxy. This keeps:
- the always-on context minimal,
- secrets out of the agent,
- the integration layer independent of any claude.ai account,
- the whole thing SaaS-portable.
