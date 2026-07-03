/**
 * Calendar Module
 * Google Calendar via owned Google OAuth (self-hosted Nango). Reads/sync
 * are free; create/move are HUMAN-GATED (docs/SECURITY.md §2).
 */
osRegisterModule({
  id: "calendar",
  name: "Calendar",
  icon: "Calendar",
  color: "var(--neo-green)",
  num: 7,
  version: "1.0.0",

  entityTypes: {
    calendar_event: {
      label: "Event",
      plural: "Events",
      icon: "Calendar",
      attrs: {
        title: { type: "text", required: true },
        start: { type: "text", required: true },
        end: { type: "text", required: true },
        attendees: { type: "text", required: false },
        location: { type: "text", required: false },
        recurrence: { type: "text", required: false },
        source_uid: { type: "text", required: false }
      },
      display: {
        title: "title",
        subtitle: "location"
      }
    }
  },

  views: [
    { id: "calendar", label: "Calendar", kind: "calendar", type: "calendar_event" },
    { id: "agenda", label: "Agenda", kind: "list", type: "calendar_event" }
  ],

  events: ["cal.synced", "cal.created", "cal.updated"],

  agentTools: [
    { name: "cal.sync", schema: {}, impl: "sync", gated: false },
    { name: "cal.read", schema: {}, impl: "list", gated: false },
    { name: "cal.create", schema: {}, impl: "create", gated: true }, // GATED
    { name: "cal.move", schema: {}, impl: "move", gated: true } // GATED
  ],

  integrations: ["google-calendar"]
});
