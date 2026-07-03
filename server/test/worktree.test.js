import { execFile as execFileCb } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { commitAndMerge, createWorktree, removeWorktree } from "../lib/worktree.js";

const execFile = promisify(execFileCb);

let repoRoot;

async function git(args, cwd = repoRoot) {
  return execFile("git", args, { cwd });
}

beforeEach(async () => {
  repoRoot = await fs.mkdtemp(path.join(os.tmpdir(), "lifeos-scaffold-test-"));
  await git(["init", "-b", "main"]);
  await git(["config", "user.email", "test@example.com"]);
  await git(["config", "user.name", "Test"]);
  await fs.writeFile(path.join(repoRoot, "README.md"), "scratch repo\n");
  await git(["add", "README.md"]);
  await git(["commit", "-m", "initial commit"]);
});

afterEach(async () => {
  await fs.rm(repoRoot, { recursive: true, force: true });
});

describe("createWorktree / removeWorktree / commitAndMerge", () => {
  it("creates a worktree on a fresh branch off main", async () => {
    const { worktreePath, branch } = await createWorktree(repoRoot, "reading");

    expect(branch).toBe("scaffold-reading");
    const stat = await fs.stat(worktreePath);
    expect(stat.isDirectory()).toBe(true);

    const { stdout } = await git(["branch", "--list", branch]);
    expect(stdout).toContain(branch);
  });

  it("commits inside the worktree and fast-forwards main onto it", async () => {
    const { worktreePath, branch } = await createWorktree(repoRoot, "reading");

    await fs.mkdir(path.join(worktreePath, "modules", "reading"), { recursive: true });
    await fs.writeFile(path.join(worktreePath, "modules", "reading", "module.js"), "osRegisterModule({});\n");

    await commitAndMerge(repoRoot, worktreePath, branch, "reading");

    const { stdout: log } = await git(["log", "--oneline", "-1", "main"]);
    expect(log).toContain("feat: self-extension - reading module");

    const installed = await fs.readFile(path.join(repoRoot, "modules", "reading", "module.js"), "utf8");
    expect(installed).toContain("osRegisterModule");
  });

  it("removes the worktree directory and branch", async () => {
    const { worktreePath, branch } = await createWorktree(repoRoot, "reading");

    await removeWorktree(repoRoot, worktreePath, branch);

    await expect(fs.stat(worktreePath)).rejects.toThrow();
    const { stdout } = await git(["branch", "--list", branch]);
    expect(stdout.trim()).toBe("");
  });
});
