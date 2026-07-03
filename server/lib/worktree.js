// Worktree isolation + install-as-commit (docs/SELF-EXTENSION.md §5).
// `execFile` with an argument array throughout - never `exec` with string
// interpolation, so a module id/prompt can never inject shell syntax.
import { execFile as execFileCb } from "node:child_process";
import path from "node:path";
import { promisify } from "node:util";

const execFile = promisify(execFileCb);

function branchName(moduleId) {
  return `scaffold-${moduleId}`;
}

export async function createWorktree(repoRoot, moduleId) {
  const branch = branchName(moduleId);
  const worktreePath = path.join(repoRoot, ".claude", "worktrees", branch);

  await execFile("git", ["worktree", "add", "-b", branch, worktreePath, "main"], { cwd: repoRoot });

  return { worktreePath, branch };
}

// Best-effort: a failed branch delete (e.g. already gone) shouldn't mask an
// otherwise-successful worktree removal - this always runs to completion on
// both the success and failure paths of scaffoldModule.
export async function removeWorktree(repoRoot, worktreePath, branch) {
  await execFile("git", ["worktree", "remove", "--force", worktreePath], { cwd: repoRoot });
  await execFile("git", ["branch", "-D", branch], { cwd: repoRoot }).catch(() => {});
}

// Commits inside the worktree, then fast-forwards `main` onto it. The
// branch was cut from `main`'s own tip in createWorktree and nothing else
// merges into `main` mid-build, so `--ff-only` always applies here - if it
// ever doesn't, that's a real conflict worth surfacing loudly, not papering
// over with a merge commit.
export async function commitAndMerge(repoRoot, worktreePath, branch, moduleId) {
  await execFile("git", ["add", path.join("modules", moduleId)], { cwd: worktreePath });
  await execFile("git", ["commit", "-m", `feat: self-extension - ${moduleId} module`], { cwd: worktreePath });
  await execFile("git", ["merge", "--ff-only", branch], { cwd: repoRoot });
}
