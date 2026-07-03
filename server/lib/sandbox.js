// Layer C (docs/SELF-EXTENSION.md §2) - the macOS Seatbelt kernel backstop
// for any Bash child the agent spawns. Read/Edit/Write bypass Seatbelt
// entirely (that's why Layer B's PreToolUse hook is the real guarantee for
// file writes) - this only confines what a shell subprocess can touch.
// `failIfUnavailable: true` makes the build refuse to run rather than
// silently proceed unsandboxed if Seatbelt can't initialize.
export function buildSandboxConfig() {
  return {
    sandbox: {
      enabled: true,
      failIfUnavailable: true,
      allowUnsandboxedCommands: false,
      filesystem: { allowWrite: ["./modules"] },
      credentials: {
        files: [
          { path: "~/.aws", mode: "deny" },
          { path: "~/.ssh", mode: "deny" },
        ],
        envVars: [
          { name: "GITHUB_TOKEN", mode: "deny" },
          { name: "NPM_TOKEN", mode: "deny" },
        ],
      },
    },
  };
}
