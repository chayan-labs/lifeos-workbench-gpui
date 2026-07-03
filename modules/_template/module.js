/**
 * Template Module Manifest for Life OS Self-Extension.
 * Used by server/scaffold.js to bootstrap net-new domains.
 */
osRegisterModule({
  id: "template_id",
  name: "Template Module",
  icon: "Zap",
  color: "var(--neo-yellow)",
  num: 99,
  version: "1.0.0",

  entityTypes: {
    item: {
      label: "Item",
      plural: "Items",
      icon: "FileText",
      attrs: {
        name: { type: "text", required: true },
        notes: { type: "text", required: false }
      },
      display: {
        title: "name",
        subtitle: "notes"
      },
      lifecycle: ["active", "archived"]
    }
  },

  views: [
    {
      id: "all",
      label: "All Items",
      kind: "list",
      type: "item",
      sortBy: "created_at"
    }
  ],

  events: ["item.created", "item.archived"],
  botCommands: [
    { cmd: "add", help: "Add a new item to template module", handler: "handleAdd" }
  ],
  agentTools: [
    { name: "template.add", schema: {}, impl: "handleAdd", gated: false }
  ]
});
