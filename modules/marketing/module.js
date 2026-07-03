/**
 * Marketing Module
 * Connects ad campaigns, content scheduling, leads, and analytics funnel dashboards.
 * Outward sends are HUMAN-GATED.
 */
osRegisterModule({
  id: "marketing",
  name: "Marketing & Campaigns",
  icon: "Megaphone",
  color: "var(--neo-yellow)",
  num: 6,
  version: "1.0.0",

  entityTypes: {
    campaign: {
      label: "Campaign",
      plural: "Campaigns",
      icon: "Flag",
      attrs: {
        goal: { type: "text", required: true },
        budget: { type: "number", required: false },
        start: { type: "date", required: false },
        end: { type: "date", required: false }
      },
      display: {
        title: "title",
        badge: "status"
      },
      lifecycle: ["planning", "active", "completed"]
    }
  },

  views: [
    { id: "campaigns_list", label: "Active Campaigns", kind: "list", type: "campaign" }
  ],

  events: ["campaign.launched", "content.sent"],

  botCommands: [
    { cmd: "campaign", help: "Show campaign analytics status", handler: "campaign_status" }
  ],

  agentTools: [
    { name: "marketing.draft", schema: {}, impl: "draft_content", gated: false },
    { name: "marketing.send", schema: {}, impl: "send_broadcast", gated: true } // GATED
  ]
});
