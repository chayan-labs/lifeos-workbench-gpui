import path from "node:path";
import { describe, expect, it } from "vitest";
import { createPreToolUseHook, isPathAllowed } from "../lib/preToolUseHook.js";

const TARGET = path.resolve("/repo/modules/foo");

describe("isPathAllowed", () => {
  it("allows the target directory itself", () => {
    expect(isPathAllowed(TARGET, TARGET)).toBe(true);
  });

  it("allows a file strictly under the target directory", () => {
    expect(isPathAllowed(TARGET, path.join(TARGET, "module.js"))).toBe(true);
    expect(isPathAllowed(TARGET, path.join(TARGET, "nested", "file.js"))).toBe(true);
  });

  it("denies a sibling module directory", () => {
    expect(isPathAllowed(TARGET, path.resolve("/repo/modules/bar/module.js"))).toBe(false);
  });

  it("denies a prefix-match trap (modules/foo_bar vs modules/foo)", () => {
    expect(isPathAllowed(TARGET, path.resolve("/repo/modules/foo_bar/module.js"))).toBe(false);
  });

  it("denies path traversal that resolves outside the target", () => {
    expect(isPathAllowed(TARGET, path.join(TARGET, "..", "..", "..", "etc", "passwd"))).toBe(false);
  });

  it("denies an absolute path elsewhere entirely", () => {
    expect(isPathAllowed(TARGET, "/etc/passwd")).toBe(false);
  });
});

describe("createPreToolUseHook", () => {
  it("returns {} (defer/allow) for a write inside the target dir", async () => {
    const hook = createPreToolUseHook(TARGET);
    const result = await hook({ tool_input: { file_path: path.join(TARGET, "module.js") } });
    expect(result).toEqual({});
  });

  it("denies with the documented hookSpecificOutput shape for an escape attempt", async () => {
    const hook = createPreToolUseHook(TARGET);
    const result = await hook({ tool_input: { file_path: "/etc/passwd" } });

    expect(result).toEqual({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: "writes confined to the new module dir",
      },
    });
  });

  it("defers when there's no file_path to check (e.g. a Bash tool call)", async () => {
    const hook = createPreToolUseHook(TARGET);
    expect(await hook({ tool_input: { command: "echo hi" } })).toEqual({});
  });
});
