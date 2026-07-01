# Security & permissions

Layered, fail-closed boundaries. Auditability over speed; gate the irreversible.

---

## 1. Permission boundary (hard, layered)

- **Common DB:** harness = full RW; Haiku bot = full RW **within its workspace**.
- **Trading: read-only for any agent/bot.** No order tool registered anywhere. A fail-closed `broker-guard` PreToolUse hook denies place/modify/cancel/GTT even if the Kite MCP is mis-loaded; broker keys are read-scoped. Orders flow agent → `proposed_order` entity → Telegram approve → a **separate interactive `trade-exec`** (never agent/hook/cron-callable; typed confirmation). **No autonomous trading.**
- **Outward actions (social/marketing publish, email send, calendar write, drive share, browser actions):** `gated` agent tools produce **drafts only**; publishing requires Telegram (or PWA) approval, then a Worker/Mac executor performs the provider call.
- **Secrets:** OAuth tokens live in **Nango** (encrypted), the agent holds only a `connectionId`; the few non-Nango secrets (Kite daily token, WhatsApp) are envelope-encrypted in `connections.secret_enc`. **Never in agent context, never in logs.**
- **`events` append-only:** no UPDATE/DELETE route, so even the RW token cannot rewrite history.

**Implemented (issue #70):** `worker/src/bot.ts` registers a `bot.use` middleware, ahead of
every `bot.command(...)` handler, that inserts `events(type='bot.command', actor='bot',
attrs={text})` for every incoming `/command` - so bot activity (not just #66's
approve/deny transitions, which already recorded their own richer-typed events) is fully
reconstructable from `events`. Only ever an INSERT, matching the append-only rule above.

---

## 2. The gating state machine

```
agent tool (gated:true) ──► draft entity + events('*.drafted')
                              │
                              ▼
              Telegram/PWA approve  ──deny──► events('*.rejected'), stop
                              │ approve
                              ▼
        Mac/Worker executor ──► Nango proxy / browser actuator / trade-exec
                              ▼
                        events('*.published'|'*.sent'|'*.executed')
```
Every transition is an `event`. Nothing outward happens without a human approve.

**Implemented (issue #66):** the Telegram half of this diagram, up to (not including) the
executor. `worker/src/approvals.ts` - `approveEntity`/`denyEntity` take a `pending_approval`
entity id, verify it's still `pending_approval` **for the caller's workspace** before
transitioning (a stale/duplicate tap is a safe no-op, `outcome: 'already_resolved'`, not a
crash or a double-transition), then:
- **approve** → `status='approved'`, `events('${type}.approved')`, and enqueues
  `jobs(kind='execute_approval', payload={entity_id, entity_type})` - the Worker never calls
  Nango's proxy, the browser actuator, or trade-exec itself (it holds no provider tokens,
  ARCHITECTURE.md §3.1); real dispatch of that job is `services/lifeos-drain`'s
  `dispatch()`, not yet wired for this kind (its other kinds are stubs too) - **so "approve"
  today means "queued," not "executed."** The acceptance bar ("nothing outward executes
  without an approve tap") holds either way: nothing executes at all yet without one.
- **deny** → `status='denied'`, `events('${type}.rejected')` (this doc's exact `*.rejected`
  naming), no job.

`worker/src/bot.ts`: `/draft <text>` creates a draft and attaches an inline Approve/Deny
keyboard to its own confirmation message; `/pending` lists every `pending_approval` entity
in the workspace (one message + keyboard per item, including ones `draft_action`-backed
routes in `lifeos-api` created, not just bot-originated ones) since there's no push
notification yet when a new draft appears - deferred alongside the digest in
`docs/PLATFORM-SYSTEMS.md` §3. A `callback_query:data` handler parses `approve:<id>`/
`deny:<id>`, calls the above, and edits the message + answers the callback with the
outcome.

---

## 3. Self-extension & marketplace sandbox

Codegen and untrusted manifests run under three layers ([SELF-EXTENSION.md](./SELF-EXTENSION.md) §2):
1. `allowedTools`/`disallowedTools` + `permissionMode:"dontAsk"` (hard-deny).
2. PreToolUse hook confining writes to `modules/<id>/` (absolute, holds under bypass).
3. macOS Seatbelt sandbox (`failIfUnavailable:true`) confining Bash; credential files/env denied.
Plus: only `modules/` is writable (never `core/`); every install is a git commit (one `git revert` away); marketplace manifests are signature-verified and re-validated locally.

**Implemented (issue #72):** `server/scaffold.js` + `server/lib/{preToolUseHook,sandbox,
worktree}.js` build layers 1-3 above and the worktree/commit-as-install plumbing, driving a
real `query()` from `@anthropic-ai/claude-agent-sdk`. `server/test/preToolUseHook.test.js`
proves layer 2's "confines writes to `modules/<id>/`, holds under bypass" guarantee directly
(prefix-match traps, path traversal, and absolute-path escapes all denied) without needing
the SDK. The two validators this section's last sentence implies ("every install is a git
commit") still gate on are not wired in yet - `docs/SELF-EXTENSION.md`'s #72 note has the
full real-vs-deferred breakdown.

**Implemented (issue #73):** the install-as-commit step above is now additionally gated on
a schema-valid manifest - `server/lib/moduleManifest.js`'s Zod `ModuleManifest` drives
`options.outputFormat` on the same `query()` call, and a failed `safeParse` (or an id that
disagrees with the sandboxed directory) aborts the build before anything is committed, the
same fail-closed path as a Layer B hook denial. See `docs/SELF-EXTENSION.md` §3's note.

---

## 4. Browser actuator containment

- Mac-only (trusted host), loaded on-demand, unloaded after.
- Sessions/cookies encrypted at rest like `connections`; never in agent context.
- Every state-changing action is gated; reads/scrapes are free.
- It can do anything a logged-in you can - therefore it is **never** allowed an un-gated outward action.

---

## 5. Tenancy isolation

- Every query is `workspace_id`-scoped at the API layer (RLS-style); a second workspace cannot see the first's rows.
- SaaS path: Turso database-per-workspace; per-workspace envelope key for non-Nango secrets.
- Derived state (`lifeos-derived.db`) and blobs (CAS) are local/keyed and never leak cross-workspace.

---

## 6. Must-pass verification (security)

- Order attempt via agent **and** via a mis-loaded Kite MCP → `broker-guard` denies both; no order tool registered.
- `events` UPDATE/DELETE → 404.
- Social publish / email send blocked without approval; reads succeed.
- Tokens absent from agent context and logs; a leaked-token grep finds nothing.
- A second workspace cannot read the first's entities.
- Break the scaffold template → validator fails cleanly, no partial register, worktree discarded.
