/**
 * Coding / Projects Module
 * Tracks software repositories, issues, PRs, and build states.
 */
osRegisterModule({
  id: "projects",
  name: "Coding & Projects",
  icon: "Code",
  color: "var(--neo-blue)",
  num: 3,
  version: "1.0.0",

  entityTypes: {
    project: {
      label: "Project",
      plural: "Projects",
      icon: "Terminal",
      attrs: {
        path: { type: "text", required: true },
        remote: { type: "text", required: false },
        default_branch: { type: "text", required: true, default: "main" },
        ci_state: { type: "text", required: false }
      },
      display: {
        title: "title",
        subtitle: "path",
        badge: "ci_state"
      }
    }
  },

  views: [
    { id: "projects_table", label: "Active Codebases", kind: "table", type: "project" }
  ],

  events: ["repo.scanned", "ci.observed", "review.requested"],

  botCommands: [
    { cmd: "proj", help: "View active workspace project statuses", handler: "status" }
  ],

  agentTools: [
    { name: "proj.status", schema: {}, impl: "status", gated: false },
    { name: "proj.scan", schema: {}, impl: "scan", gated: false }
  ]
});
