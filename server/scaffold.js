// Self-extension builder (issues #72/#73/#74/#75, docs/SELF-EXTENSION.md).
// Drives the Claude Agent SDK, tool-restricted by all three defense-in-depth
// layers (§2), in an isolated git worktree, requires a schema-valid
// structured-output manifest (§3), passes both validators (§4), and commits
// the result as the install (§5).
//
// `slugify()` still picks the module id *before* `query()` runs (Layer B's
// hook needs a concrete target directory up front), but the agent's own
// structured-output manifest.id is now asserted to match it - drift between
// the two fails the build rather than silently installing a mismatched
// manifest.
//
import fs from "node:fs/promises";
import path from "node:path";
import { query as defaultQuery } from "@anthropic-ai/claude-agent-sdk";
import { ModuleManifest, moduleManifestJsonSchema } from "./lib/moduleManifest.js";
import { buildSandboxConfig } from "./lib/sandbox.js";
import { createPreToolUseHook } from "./lib/preToolUseHook.js";
import { slugify } from "./lib/slugify.js";
import { commitAndMerge, createWorktree, removeWorktree } from "./lib/worktree.js";
import { validateStructural } from "./validators/structural.js";
import { validateRenderSmoke as defaultValidateRenderSmoke } from "./validators/render.js";

const DEFAULT_REPO_ROOT = path.resolve(import.meta.dirname, "..");

// Layer A (docs/SELF-EXTENSION.md §2) - the primary gate. `dontAsk` denies
// anything not pre-approved instead of prompting, which is what makes this
// safe to run headless/unattended. Never `bypassPermissions` - it isn't
// constrained by `allowedTools` at all.
const ALLOWED_TOOLS = ["Read", "Glob", "Grep", "Edit", "Write", "Bash"];
const DISALLOWED_TOOLS = ["WebFetch", "WebSearch", "Bash(rm -rf *)", "Bash(git push *)", "Bash(curl *)"];

async function copyTemplate(repoRoot, worktreePath, moduleId) {
  const templateDir = path.join(repoRoot, "modules", "_template");
  const targetModuleDir = path.join(worktreePath, "modules", moduleId);
  await fs.cp(templateDir, targetModuleDir, { recursive: true });
  return targetModuleDir;
}

function buildPrompt(userPrompt, moduleId) {
  return [
    `Edit modules/${moduleId}/module.js (already seeded from the _template scaffold) so it satisfies this request:`,
    userPrompt,
    `Keep it a single osRegisterModule({...}) call, id: "${moduleId}". Only edit files under modules/${moduleId}/.`,
    `When done, your structured output must summarize the manifest you wrote: id, name, icon, color, entityTypes (with attrs), views, botCommands, and agentTools - matching the id "${moduleId}" exactly.`,
  ].join("\n\n");
}

// Consumes the SDK's async-generator result stream, watching for a denied
// tool call (tracked via the hook wrapper below, not by re-parsing SDK
// messages - the hook already knows the ground truth) and a terminal
// success/error `result` message carrying the schema-validated structured
// output (docs/SELF-EXTENSION.md §3). Returns the parsed ModuleManifest.
async function runAgent(queryFn, prompt, options, hookState, moduleId) {
  const stream = queryFn({ prompt, options });
  let resultMessage = null;

  for await (const message of stream) {
    if (message.type === "result") {
      resultMessage = message;
    }
  }

  if (hookState.denied) {
    throw new Error(`PreToolUse hook denied a write outside the module dir: ${hookState.reason}`);
  }
  if (!resultMessage || resultMessage.subtype !== "success" || resultMessage.is_error) {
    // Covers error_during_execution / error_max_turns / error_max_budget_usd
    // and the SDK's own structured-output retry exhaustion.
    throw new Error(`Agent SDK query did not complete successfully (subtype: ${resultMessage?.subtype ?? "none"})`);
  }

  const parsed = ModuleManifest.safeParse(resultMessage.structured_output);
  if (!parsed.success) {
    throw new Error(`Structured output failed ModuleManifest validation: ${parsed.error.message}`);
  }
  if (parsed.data.id !== moduleId) {
    throw new Error(`Structured output id "${parsed.data.id}" does not match target module id "${moduleId}"`);
  }

  return parsed.data;
}

export async function scaffoldModule(prompt, workspaceId, opts = {}) {
  const repoRoot = opts.repoRoot ?? DEFAULT_REPO_ROOT;
  const queryFn = opts.queryFn ?? defaultQuery;
  const validateRenderSmoke = opts.validateRenderSmoke ?? defaultValidateRenderSmoke;

  const moduleId = slugify(prompt);
  const { worktreePath, branch } = await createWorktree(repoRoot, moduleId);

  try {
    const targetModuleDir = await copyTemplate(repoRoot, worktreePath, moduleId);

    // Wraps Layer B's hook so scaffold.js can observe a denial directly,
    // rather than inferring it from the SDK's message stream.
    const hookState = { denied: false, reason: null };
    const baseHook = createPreToolUseHook(targetModuleDir);
    const trackedHook = async (input) => {
      const result = await baseHook(input);
      if (result.hookSpecificOutput?.permissionDecision === "deny") {
        hookState.denied = true;
        hookState.reason = result.hookSpecificOutput.permissionDecisionReason;
      }
      return result;
    };

    const options = {
      cwd: worktreePath,
      allowedTools: ALLOWED_TOOLS,
      disallowedTools: DISALLOWED_TOOLS,
      permissionMode: "dontAsk",
      hooks: { PreToolUse: [{ matcher: "Write|Edit", hooks: [trackedHook] }] },
      outputFormat: { type: "json_schema", schema: moduleManifestJsonSchema },
      ...buildSandboxConfig(),
    };

    const manifest = await runAgent(queryFn, buildPrompt(prompt, moduleId), options, hookState, moduleId);

    // Validator 1 (§4, issue #74) - re-loads the file the agent actually
    // wrote (not the structured-output summary) and checks it against
    // module.schema.json, plus dup-type-id and dangling-view-ref checks
    // against the worktree's full modules/ tree (a worktree checkout already
    // contains every sibling module, so no separate lookup is needed).
    const structural = await validateStructural(path.join(targetModuleDir, "module.js"), {
      modulesDir: path.join(worktreePath, "modules"),
    });
    if (!structural.valid) {
      throw new Error(`Structural validation failed: ${structural.errors.join("; ")}`);
    }

    // Validator 2 (§4, issue #75) - boots the real app stack (its own
    // default repoRoot, not the worktree/scratch `repoRoot` above: the
    // frontend build and lifeos-api binary only exist in the real checkout,
    // and the check only needs `moduleId` + the manifest's `name`, not any
    // worktree-specific file - see server/validators/render.js's header).
    const render = await validateRenderSmoke(moduleId, manifest);
    if (!render.valid) {
      throw new Error(`Render smoke validation failed: ${render.errors.join("; ")}`);
    }

    await commitAndMerge(repoRoot, worktreePath, branch, moduleId);
    await removeWorktree(repoRoot, worktreePath, branch);

    return { success: true, moduleId, workspaceId, manifest };
  } catch (error) {
    await removeWorktree(repoRoot, worktreePath, branch).catch(() => {});
    return { success: false, moduleId, workspaceId, error: error.message };
  }
}

// Manual local smoke run - not exercised by the test suite (needs a real
// ANTHROPIC_API_KEY and mutates real git state), see docs/SELF-EXTENSION.md's
// "Implemented (issue #72)" note.
if (process.argv[1] === import.meta.filename) {
  const result = await scaffoldModule("add a reading list module", "default-personal-workspace");
  console.log(result);
}
