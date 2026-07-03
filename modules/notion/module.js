/**
 * Notion Module
 * Two-way sync via owned Notion OAuth (self-hosted Nango), docs/MODULES.md
 * §3.4. Reads/sync are free; pushing local edits back to Notion is
 * HUMAN-GATED (docs/SECURITY.md §2) - it only ever drafts a pending update.
 */
osRegisterModule({
  id: "notion",
  name: "Notion",
  icon: "StickyNote",
  color: "var(--neo-purple)",
  num: 9,
  version: "1.0.0",

  entityTypes: {
    note: {
      label: "Note",
      plural: "Notes",
      icon: "FileText",
      attrs: {
        notion_id: { type: "text", required: true },
        mirrors: { type: "text", required: true }
      },
      display: {
        title: "title"
      }
    },
    notion_page: {
      label: "Notion Page",
      plural: "Notion Pages",
      icon: "File",
      attrs: {
        notion_id: { type: "text", required: true },
        title: { type: "text", required: false },
        last_edited_time: { type: "text", required: false }
      },
      display: {
        title: "title"
      }
    },
    notion_db: {
      label: "Notion Database",
      plural: "Notion Databases",
      icon: "Database",
      attrs: {
        notion_id: { type: "text", required: true }
      },
      display: {
        title: "title"
      }
    }
  },

  views: [
    { id: "notes", label: "Notes", kind: "list", type: "note" },
    { id: "pages", label: "Mirrored Pages", kind: "list", type: "notion_page" },
    { id: "databases", label: "Databases", kind: "list", type: "notion_db" }
  ],

  events: ["note.synced"],

  agentTools: [
    { name: "note.sync", schema: {}, impl: "sync", gated: false },
    { name: "note.read", schema: {}, impl: "list", gated: false },
    { name: "notion.create", schema: {}, impl: "create", gated: true }, // GATED
    { name: "notion.push", schema: {}, impl: "push", gated: true } // GATED
  ],

  integrations: ["notion"]
});
