/**
 * Social Module
 * Multi-account management for Instagram, X, Reddit, Slack, and WhatsApp.
 * Uses self-hosted Nango for credential proxying.
 * All outward writes are HUMAN-GATED.
 */
osRegisterModule({
  id: "social",
  name: "Social accounts",
  icon: "Globe",
  color: "var(--neo-mint)",
  num: 5,
  version: "1.0.0",

  entityTypes: {
    social_account: {
      label: "Social Account",
      plural: "Social Accounts",
      icon: "User",
      attrs: {
        provider: { type: "text", required: true },
        handle: { type: "text", required: true },
        nango_connection_id: { type: "text", required: true }
      },
      display: {
        title: "handle",
        subtitle: "provider"
      }
    },
    post: {
      label: "Post Draft",
      plural: "Posts",
      icon: "Share2",
      attrs: {
        body: { type: "text", required: true },
        scheduled_for: { type: "date", required: false }
      },
      display: {
        title: "body",
        badge: "status"
      },
      lifecycle: ["draft", "approved", "published"]
    }
  },

  views: [
    { id: "social_inbox", label: "Unified Inbox", kind: "list", type: "post" },
    { id: "content_calendar", label: "Post Calendar", kind: "calendar", type: "post" }
  ],

  events: ["post.drafted", "post.published", "mention.received", "dm.received"],

  botCommands: [
    { cmd: "inbox", help: "View direct messages and social mentions", handler: "inbox" },
    { cmd: "draft", help: "Draft a post: /draft <body>", handler: "draft_post" }
  ],

  agentTools: [
    { name: "social.draft", schema: {}, impl: "draft", gated: false },
    { name: "social.publish", schema: {}, impl: "publish", gated: true } // GATED
  ]
});
