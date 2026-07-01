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

**Validator 2 - render smoke (headless Playwright):**
- Boot the app against a **scratch derived/replica DB** (never canonical `lifeos.db`), on an **ephemeral port**.
- Mount the new tile; assert **0 console/page JS errors** for the full session (`page.on('console'|'pageerror')`); assert each declared view mounts a node; assert an app-emitted **`module-mounted:<id>`** ready event (not arbitrary timeouts).
- **One bounded retry** before declaring failure (a single transient render error shouldn't burn a valid module).
- Fail → discard the worktree.

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
