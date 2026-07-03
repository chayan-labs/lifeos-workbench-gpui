/**
 * Tasks / Productivity Module
 * Handles projects, personal tasks, scheduling, and Kanban boards.
 */
osRegisterModule({
  id: "tasks",
  name: "Tasks & Productivity",
  icon: "CheckSquare",
  color: "var(--neo-yellow)",
  num: 2,
  version: "1.0.0",

  entityTypes: {
    task: {
      label: "Task",
      plural: "Tasks",
      icon: "CheckCircle",
      attrs: {
        due: { type: "date", required: false },
        priority: { type: "enum", enum: ["high", "medium", "low"], required: true },
        estimate: { type: "number", required: false }, // hours
        tags: { type: "text", required: false }
      },
      display: {
        title: "title",
        subtitle: "due",
        badge: "priority"
      },
      lifecycle: ["todo", "in_progress", "completed", "blocked"]
    }
  },

  views: [
    { id: "kanban", label: "Task Board", kind: "board", type: "task", groupBy: "status" },
    { id: "today", label: "My Today", kind: "list", type: "task", filter: "status = 'in_progress'" }
  ],

  events: ["task.created", "task.completed", "task.blocked"],

  botCommands: [
    { cmd: "task", help: "Log a task: /task <title>", handler: "task_create" },
    { cmd: "done", help: "Complete a task: /done <id>", handler: "task_complete" }
  ],

  agentTools: [
    { name: "task.create", schema: {}, impl: "create", gated: false },
    { name: "task.complete", schema: {}, impl: "complete", gated: false }
  ]
});
