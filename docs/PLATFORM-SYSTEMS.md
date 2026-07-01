# Platform systems

Cross-cutting systems that are not modules but make Life OS feel like one app you never leave.
All sit on the existing schema (`entities`/`edges`/`events`/`jobs`) - **zero new tables.**

---

## 1. Command bar + agent pipelines

### 1.1 Command bar (`Cmd-K`)
A single natural-language bar that routes to entities, actions, or pipelines.
- **Search:** queries the one hybrid index (FTS5 + memvec RRF) over **every module** → entities across domains.
- **Verbs:** "draft a post about X", "what's blocked", "diff this asset", "find the clip where I said Y".
- **Build:** `core/command.js` (JS, browser) over the existing search + tool registry. No new storage.

### 1.2 Agent pipelines (user/module-defined DAGs)
Multi-agent Workflows declared in the manifest:
```js
pipelines: [{ id:'post-from-topic', stages:[
  { agent:'research', tool:'memvec.recall' },
  { agent:'draft',    skill:'copywriting' },
  { agent:'verify',   gate:'eval' },
  { agent:'publish',  tool:'social.draft', gated:true } ]}]
```
- **Engine `lifeos-pipelines`:** runs on the Mac heavy lane, enqueued via `jobs`, dispatching **Claude Agent SDK** calls per stage.
- Each run = a `pipeline_run` entity; each stage writes `events` (run_id, stage, tokens, outcome) → reuses the Eval/Gate/Observe loop ([HARNESS-LOOP.md](./HARNESS-LOOP.md)).
- Outward final stages are **gated**.
- **Build:** 🦀 Rust orchestrator (or JS if kept close to `scaffold.js`); the per-stage worker is the Agent SDK.

---

## 2. Dashboards / analytics (derived from `events`)

**No new storage** - `events` is already the fact table. A manifest declares metrics; a generic agg layer renders them:
```js
metrics: [
  { id:'pnl_curve',      source:'events', where:"type='trade.closed'",   agg:'cumsum(attrs.pnl)', viz:'line' },
  { id:'posts_per_week', source:'events', where:"type='post.published'", agg:'count', bucket:'week', viz:'bar' } ]
```
- **Build:** `core/analytics.js` (JS render) + a `lifeos metrics` endpoint in the Rust API (pure SQL agg over `events` - fast).
- Cross-module dashboard = union over all manifests' metrics.
- **Time-travel "as of"** works because `events` is append-only.

---

## 3. Notifications & digest

- **Daily Telegram (and Slack) digest:** what's due, what's blocked, PnL, drafts awaiting approval, what the Life OS Actions did overnight.
- Built from a scheduled `jobs` row → query over `events`/`entities` → send via the bot.
- PWA push (§5) mirrors the same digest.

**Implemented (issue #65):** the pieces this digest will eventually assemble already exist
as standalone Telegram commands - `worker/src/commands.ts`: `/today` (due today), `/pnl`
(realized PnL), `/inbox` (uncategorized captures - status IS NULL, closest analog to
"what's blocked" until a `task.blocked` event exists), `/draft` (creates a
`pending_approval` entity, "drafts awaiting approval"). No scheduled digest job yet - that's
a `jobs`-row + cron trigger, deferred until the heavy-job enqueue path (#67) exists to model
it on. `/task`/`/topic`/`/done`/`/quiz` are capture/complete/quiz commands, not digest
inputs - see `docs/MODULES.md` §2.1/§2.2/§2.4 for their module-specific notes. Every command
is workspace-scoped (`worker/src/db.ts::resolveWorkspaceId`, #64) and reads/writes only via
`entities.ts`/`events.ts`, never a direct query - tested against a real in-memory libSQL DB,
`worker/test/{entities,events,commands,bot}.test.ts`.

**Implemented (issue #71):** `worker/src/digest.ts::buildDigest` composes the pieces above
into one message. It's a Cloudflare **Cron Trigger** (`wrangler.toml`'s `[triggers] crons`),
not a `jobs` row - the digest is a pure DB read + a single Telegram send, no heavy/codegen
work a Mac drain would need to do, so routing it through the `jobs`/drain machinery #66/#67
built for outward-provider-call and Mac-only work would add a hop for no reason. `index.ts`
exports a `scheduled(event, env)` handler that builds and sends it; unset `DIGEST_CHAT_ID`
means no digest (manual-setup-gated the same way `BOT_TOKEN`/`TURSO_URL`/etc. are,
`docs/MANUAL-SETUP.md`). `buildDigest` is fully unit-tested (`worker/test/digest.test.ts`);
the actual Telegram send is verified live post-deployment, same as `/telegram` itself. PWA
push mirroring this digest is Phase 7, not yet built.

---

## 4. Module marketplace (the SaaS seam)

A module is already a validated, self-contained manifest ([SELF-EXTENSION.md](./SELF-EXTENSION.md)). The marketplace distributes and installs them.
- **Entity types:** `module_package` (`{id, version, author, manifest_ref(blob), signature, installs}`), `module_review`.
- **Publish:** `lifeos module publish` → structural + render validators → **sign** the manifest (ed25519, tamper-evident) → push to a registry (a Turso table + R2 blob, or a GitHub repo of manifests).
- **Install:** the **same two validators run locally on the trusted Mac** before register → git commit. Untrusted manifests get the same `dontAsk` + PreToolUse + Seatbelt sandbox treatment as self-built ones.
- **Security:** never auto-install into `core/`; manifests are declarative (no arbitrary code), drastically limiting blast radius; signature + local re-validation + sandbox.
- **Build:** 🦀 Rust for sign/verify (ed25519) inside the `lifeos` API; validators reused from Phase 5.
- This is the multi-tenant distribution channel.

---

## 5. PWA (rich mobile beyond Telegram)

- The SPA + `manifest.webmanifest` + a **service worker** (offline cache of `core/` + the embedded-replica read model) + **Web Push**.
- Telegram stays for quick capture/approve; the PWA is the full-fidelity surface (galleries, dashboards, version diffs, maps, timelines).
- **Auth:** `frontend/src/lib` auth path (session + workspace) - no-op locally, real for SaaS.
- **Offline:** reads from a local cache / IndexedDB mirror; writes spool to `store/` and reconcile via the single-writer + events-as-truth model ([DATA-MODEL.md](./DATA-MODEL.md) §4).
- **Build:** JS/PWA; the Worker serves push.

---

## 6. Life OS Actions (event-triggered automation - "GitHub Actions for everything")

Declared per module as `actions: [{ on, if?, run }]`; a rules engine over `events` → `jobs`.
Examples:
- `on asset.version_created` → generate thumbnail + caption + draft social post.
- `on design_file.updated` → regenerate code component (figma-implement-design) + open a PR.
- `on trade.closed` → update equity curve + draft journal reflection.
- `on topic.due` → quiz in Telegram.
- **Engine:** 🦀 Rust hot event loop (part of `lifeos-pipelines` or standalone). Outward steps gated; internal steps free.
- New automations need **zero new code** - just manifest entries.
