// Layer B (docs/SELF-EXTENSION.md §2) - the code-level guarantee that holds
// even if Layer A (allowedTools/permissionMode) is ever misconfigured or
// bypassed. `allowedTools` can't express "Write only under modules/<id>/",
// so this PreToolUse hook does: it denies any Write/Edit/MultiEdit whose
// `file_path` resolves outside the target module directory. Isolated from
// scaffold.js so it's unit-testable without the Agent SDK.
import path from "node:path";

// Strict prefix match, `path.sep`-bounded, so a sibling directory that
// merely starts with the same characters (`modules/foo_bar` when the target
// is `modules/foo`) is never mistaken for "inside."
export function isPathAllowed(targetModuleDir, filePath) {
  const resolvedTarget = path.resolve(targetModuleDir);
  const resolvedFile = path.resolve(filePath);
  return resolvedFile === resolvedTarget || resolvedFile.startsWith(resolvedTarget + path.sep);
}

export function createPreToolUseHook(targetModuleDir) {
  return async (input) => {
    const filePath = input?.tool_input?.file_path;
    if (typeof filePath !== "string" || isPathAllowed(targetModuleDir, filePath)) {
      return {};
    }

    return {
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: "writes confined to the new module dir",
      },
    };
  };
}
