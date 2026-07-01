# Self-extension - "Ask AI to add a module"

> The system grows itself: an in-app affordance that, on request, writes a brand-new declarative module, validates it, and hot-loads it - as a revertable git commit.
> Built on the **Claude Agent SDK** (not a raw `claude -p` subprocess), so tool-locking, structured output, and hooks are first-class.

---

## 1. Flow (Mac online, synchronous)

1. `POST /api/module-request { prompt, workspace_id }` → `module_requests` row + `events('module.requested')`.
2. `server/scaffold.js` drives the **Claude Agent SDK** (`@anthropic-ai/claude-agent-sdk`, `query({prompt, options})`), tool-restricted (see §2), given `modules/_template/` and existing manifests as examples.
3. The agent copies `modules/_template` → `modules/<id>/`, fills the manifest (entity types, attrs, views, integrations, bot commands, light color), and emits a **schema-validated manifest summary** as structured output (§3).
4. **Two validators** run (§4). Both must pass.
5. Insert `modules` row + `events('module.installed')`; **SSE hot-reload** - the new tile appears, no restart.
6. On failure → `status='failed'` surfaced in-app, one retry.

## 1b. Flow (Mac offline, queued)
Telegram `/addmodule …` → the bot writes `module_requests(status='queued')` to cloud Turso, replies "queued".
A `launchd` poller on the Mac drains on wake, runs the **identical** local build, commits to git, bot notifies "✅ live".
**Codegen only ever runs on the trusted Mac; the cloud bot only enqueues.**

---

## 2. Tool restriction - defense in depth (3 layers)

Restricting writes to one subdir is not a single switch; layer all three.

**Layer A - locked tool surface (primary gate):**
```ts
options = {
  allowedTools: ["Read","Glob","Grep","Edit","Write","Bash"],
  disallowedTools: ["WebFetch","WebSearch","Bash(rm -rf *)","Bash(git push *)","Bash(curl *)"],
  permissionMode: "dontAsk",   // anything not pre-approved is DENIED, never prompts (headless-safe)
}
```
Do **not** use `bypassPermissions` - `allowedTools` does not constrain it.

**Layer B - PreToolUse hook that fails closed on path (the dir scope):**
`allowedTools` cannot express "Write only under `modules/<id>/`". A hook matching `Write|Edit` (and `Bash`) denies when `tool_input.file_path` resolves outside the target dir:
```ts
hooks: { PreToolUse: [{ matcher: "Write|Edit", hooks: [async (input) => {
  const p = path.resolve(input.tool_input.file_path);
  if (!p.startsWith(targetModuleDir + path.sep))
    return { hookSpecificOutput: { hookEventName:"PreToolUse",
      permissionDecision:"deny", permissionDecisionReason:"writes confined to the new module dir" } };
  return {};
}] }] }
```
Hooks run first; a deny is absolute - it holds even under `bypassPermissions`. This is the code-level guarantee.

**Layer C - OS sandbox (kernel backstop for Bash):**
Enable the built-in macOS Seatbelt sandbox so any shell child is physically confined:
```json
{ "sandbox": { "enabled": true, "failIfUnavailable": true, "allowUnsandboxedCommands": false,
  "filesystem": { "allowWrite": ["./modules"] },
  "credentials": { "files":[{"path":"~/.aws","mode":"deny"},{"path":"~/.ssh","mode":"deny"}],
                   "envVars":[{"name":"GITHUB_TOKEN","mode":"deny"},{"name":"NPM_TOKEN","mode":"deny"}] } } }
```
In a linked git worktree the sandbox auto-allows the shared `.git` so `git commit` works, while denying `.git/hooks` and `.git/config`. `failIfUnavailable:true` makes the build refuse to run if Seatbelt can't init. Note: built-in Read/Edit/Write bypass the sandbox - that is why Layer B's hook is required; the sandbox only confines Bash.

**Implemented (issue #72):** `server/scaffold.js` + `server/lib/{preToolUseHook,sandbox,
worktree,slugify}.js` - all three layers verbatim from this section, wired into a real
`query()` call from `@anthropic-ai/claude-agent-sdk` (confirmed against the installed
package's `sdk.d.ts`, not just this doc). Deliberately deferred to later issues, so this
file doesn't build more than #72's own checklist:
- **Module id selection** - `server/lib/slugify.js` is a naive placeholder (lowercase,
  non-alnum → `_`). Layer B's hook needs a concrete target directory *before* `query()`
  runs, so something outside the LLM call has to pick the id first; the agent's own
  structured-output `id` (§3, issue #73) is now asserted to match this slug, but does not
  replace it as the pre-agent directory choice - that chicken-and-egg constraint doesn't
  go away with structured output, it just gets a consistency check.
- **The two validators (§4, #74/#75)** are not called. `server/validators/structural.js`
  and `render.js` predate this issue and are fakes (a `content.includes('id:')`-style
  check and an unconditional `return true`, respectively, left over from an earlier
  prototype commit) - `scaffold.js` does not import them, since calling a fake validator
  would give false confidence rather than none. Gating the merge on real validators is
  #74/#75's job.
- **No live end-to-end run.** A real Agent SDK call needs a live `ANTHROPIC_API_KEY`, costs
  tokens, and (on success) merges a git commit into `main` - too high a blast radius to run
  unprompted in an assistant session. `server/test/scaffold.test.js` exercises the full
  pipeline (worktree create → agent → commit-and-merge → cleanup, and the escape-attempt /
  SDK-error abort paths) against a disposable scratch git repo with an injected mock
  `queryFn`; `server/test/preToolUseHook.test.js` proves Layer B's "escape attempts fail
  closed" guarantee directly, independent of whether a real LLM call ever ran. An actual
  `node server/scaffold.js` run against this repo is a manual follow-up, same as every other
  "verified live post-deployment" gate in this repo (`/telegram`, Nango, Kite, WhatsApp).

---

## 3. Structured output - schema-validated manifest with auto-retry

The SDK does natively what would otherwise be a hand-rolled ajv + retry loop:
```ts
options.outputFormat = { type: "json_schema", schema };  // schema from Zod: z.toJSONSchema(ModuleManifest)
```
The SDK **validates the output and re-prompts on mismatch**. On success the result carries `structured_output`; on exhaustion `subtype === "error_max_structured_output_retries"`.
Define the manifest schema in **Zod** (one source of truth), end with `ModuleManifest.safeParse(structured_output)` for end-to-end type safety.
The agent emits the manifest summary (entityTypes/attrs/views/botCommands/agentTools ids) as structured output → it becomes the input to Validator 1 without re-reading files.

**Implemented (issue #73):** `server/lib/moduleManifest.js` defines `ModuleManifest` in Zod
(entityTypes with typed attrs, views, botCommands, agentTools, plus top-level id/name/icon/
color) and derives `moduleManifestJsonSchema` via `z.toJSONSchema(ModuleManifest)`.
`server/scaffold.js` sets `options.outputFormat = { type: "json_schema", schema:
moduleManifestJsonSchema }` on the `query()` call, then on a `success` result runs
`ModuleManifest.safeParse(result.structured_output)` - a validation failure, or a manifest
`id` that disagrees with the pre-agent directory slug (§2's note above), aborts the build
and discards the worktree, same failure path as a hook denial. The SDK's own
`error_max_structured_output_retries` result subtype is treated as a hard failure, not
retried again at this layer - the SDK already exhausted its retry budget.
`server/test/moduleManifest.test.js` proves the schema's accept/reject boundary directly;
`server/test/scaffold.test.js` adds cases for an invalid manifest, an id mismatch, and
retry exhaustion, alongside #72's existing happy-path/escape-attempt/SDK-error cases - all
still against a mocked `queryFn` and scratch git repo, no live API key (same rationale as
#72's note above). The parsed `ModuleManifest` is now part of `scaffoldModule`'s success
return value, ready for Validator 1 (#74) to consume without re-reading `module.js`.

---

## 4. The two validators

**Validator 1 - structural (pure Node, no LLM):**
- Load the written `module.js` in a `vm`/worker, capture the `osRegisterModule({...})` argument.
- ajv-check against `module.schema.json`.
- Assert: schema-valid; no duplicate `type` ids across existing modules (query the registry); every `view.type` / ref resolves to a known core renderer; every `botCommand`/`agentTool` id unique.
- Fail → discard the worktree.

**Implemented (issue #74):** `server/lib/loadManifest.js` runs `module.js` in a fresh
`vm.createContext` that only exposes a capturing `osRegisterModule` stub - real file-system/
network globals aren't in scope, so a malformed or even hostile `module.js` can't do
anything but populate the captured object (or throw, which is caught and reported, not
crashed on). `server/validators/module.schema.json` + `server/validators/structural.js`
(rewritten - the earlier version was a `content.includes('id:')`-style stub left over from
an earlier prototype commit) run that captured object through ajv (`ajv`'s 2020-12 draft
build), then two checks ajv alone can't express: **duplicate entity-type ids** against every
sibling directory under `modules/` (skipping `_template` and the module's own directory,
matched by directory name - not by the manifest's own `id` field, which is exactly the field
under test) and **dangling view refs** (`view.type` must be a declared `entityTypes` key,
`view.kind` must be one of the core renderers `ModuleManifestPage.jsx` actually mounts -
`list/table/board/calendar/gallery/timeline/map/metric` - and a `metric`-kind view's
`view.metric` must resolve in `manifest.metrics`). A third check, not in the issue's literal
checklist but a natural corollary of "no duplicate type ids" using the same directory-name
signal: **the manifest's own `id` must equal its directory name** - without it, a mismatched
`id` silently can't be excluded from its own dup-check pass. `agentTool`/`botCommand` id
uniqueness (this section's last clause) is not yet asserted - out of #74's own checklist,
left for a follow-up alongside real cross-module registry queries.
`scaffold.js` now calls this validator on the file the agent actually wrote (not the §3
structured-output summary) before `commitAndMerge` - a structural failure aborts the build
exactly like a Layer B hook denial, discarding the worktree with nothing merged to `main`.
Spot-checked against all 14 real `modules/*/module.js` files: 13 pass; `modules/learning`
correctly fails on a genuine pre-existing `kind: "graph"` view (no `GenericGraph` renderer
exists in `frontend/src/core/renderers/`) - fixing that module is out of this issue's scope,
noted here as a known finding.

**Validator 2 - render smoke (headless Playwright):**
- Boot the app against a **scratch derived/replica DB** (never canonical `lifeos.db`), on an **ephemeral port**.
- Mount the new tile; assert **0 console/page JS errors** for the full session (`page.on('console'|'pageerror')`); assert each declared view mounts a node; assert an app-emitted **`module-mounted:<id>`** ready event (not arbitrary timeouts).
- **One bounded retry** before declaring failure (a single transient render error shouldn't burn a valid module).
- Fail → discard the worktree.

**Implemented (issue #75):** `server/lib/appBoot.js` boots the real stack as disposable child
processes on ephemeral ports (`server/lib/appBoot.js`'s own `getEphemeralPort()`) - the real
`lifeos-api` debug binary with `LIFEOS_DB_PATH`/`LIFEOS_DERIVED_DB_PATH` pointed at a fresh
`mkdtemp` scratch directory and `TURSO_URL`/`TURSO_TOKEN` cleared (never syncs, never touches
`lifeos.db`), and the real Vite dev server (`--host 127.0.0.1` pinned explicitly - Vite's
default `localhost` bind resolved to the IPv6 loopback first on the machine this was built on,
which then refused the IPv4 readiness poll). `server/validators/render.js` (rewritten - the
earlier version was an unconditional `return true` stub) launches headless Chromium, sets the
SPA's `life_os_loggedin` localStorage flag via `context.addInitScript` to skip the login gate,
collects `console`/`pageerror` for the full page session, `POST /api/event` seeds the exact
`module.installed` event the real self-extension install path (§1 step 5) emits (no auth
needed - `/api/event` falls back to the default workspace with no bearer token, same as the
frontend does), and races the real `module-mounted:<id>` `CustomEvent`
(`frontend/src/lib/moduleRegistry.js`) against a first-error signal and a bounded timeout - not
an arbitrary sleep, the timeout is only the safety net for a event that genuinely never fires.
One bounded retry (`MAX_ATTEMPTS = 2`) on any failure; teardown (browser, both child processes,
scratch DB dir) always runs, even on failure. `scaffold.js` calls this after Validator 1 passes
and before `commitAndMerge`.
**Scope note:** the live hot-install path only ever carries a minimal `{id, name, version,
icon}` manifest through the real SSE event - `InstalledModulePage.jsx` renders hot-installed
modules as a flat list, not through the full multi-view `ModuleManifestPage` that the 14 static
day-1 modules get. So "assert 0 console/page errors + `module-mounted:<id>` fires" is exercised
for real; "assert each declared view mounts a node" isn't yet, since there's no live per-view
render path for a hot-installed module to assert against today - that's frontend work beyond
this issue (same kind of scope line #74 drew for `agentTool`/`botCommand` id uniqueness).
Tested via `server/test/renderSmoke.test.js` (real Playwright + real HTTP fixtures standing in
for the heavy cargo/Vite boot, so the suite stays fast: happy path, retry-then-succeed,
retry-exhausted, timeout-without-firing, teardown-always-runs) and
`server/test/appBoot.test.js` (`getEphemeralPort` returns distinct, bindable ports).
`server/scripts/renderSmokeLive.js` is a manual entry point that runs the real stack end-to-end
(not part of the vitest suite) - unlike #72's Agent SDK constraint, this needed no paid API key
and no git mutation, so it was actually run live in this session against the real `reading`
module id and passed (`{"valid":true,"errors":[]}`), confirming the full boot → mount →
assert → teardown pipeline works, not just the mocked orchestration tests. Requires
`cargo build --bin lifeos-api` and `npm install` in `frontend/` to have been run first, plus a
one-time `npx playwright install chromium` (documented in `docs/MANUAL-SETUP.md`).

---

## 5. Isolation & commit (use Claude Code's own worktree feature)

Pipeline: create worktree `.claude/worktrees/scaffold-<id>` on a fresh branch → `query()` with Layers A/B/C → Validator 1 → Validator 2 → if both green, `git commit` in the worktree and merge to main (revertable single commit) → SSE push the new tile → remove worktree.
Any failure: remove the worktree; nothing touches main.

---

## 6. Reuse & risk

- **Port from** `anthropics/claude-agent-sdk-demos` (canonical `query()` + hooks + structured-output wiring). SDK repos: `claude-agent-sdk-typescript`, `…-python`. OS sandbox primitives standalone as `@anthropic-ai/sandbox-runtime`.
- Reuse `knowledge-atlas/tools/server.js` + `memory.js` as the app-boot/DB harness Validator 2 drives.
- **Biggest reliability risk: Validator 2 (render smoke) flakiness**, not the LLM. Mitigate with the ephemeral-port + fresh-scratch-DB + explicit ready-event + full-session error capture + one bounded retry already specified above. Keep the SDK's structured-output retry (free) separate from the render retry.

---

## 7. Marketplace tie-in
The same validated, signed manifests are the unit the **module marketplace** distributes; an install runs the *same two validators* locally before register. See [PLATFORM-SYSTEMS.md](./PLATFORM-SYSTEMS.md) §4.
