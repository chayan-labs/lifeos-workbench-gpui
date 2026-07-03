/**
 * Slack Module
 * Owned Slack OAuth (self-hosted Nango), docs/MODULES.md §3.5. A second
 * capture/notify surface alongside Telegram. Reads/sync are free; posting
 * is HUMAN-GATED (docs/SECURITY.md §2).
 */
osRegisterModule({
  id: "slack",
  name: "Slack",
  icon: "MessageSquare",
  color: "var(--neo-pink)",
  num: 10,
  version: "1.0.0",

  entityTypes: {
    channel: {
      label: "Channel",
      plural: "Channels",
      icon: "Hash",
      attrs: {
        channel_id: { type: "text", required: true },
        name: { type: "text", required: false }
      },
      display: {
        title: "title"
      }
    },
    message: {
      label: "Message",
      plural: "Messages",
      icon: "MessageSquare",
      attrs: {
        channel_id: { type: "text", required: true },
        user: { type: "text", required: false },
        text: { type: "text", required: false },
        ts: { type: "text", required: true }
      },
      display: {
        title: "text",
        subtitle: "user"
      }
    }
  },

  views: [
    { id: "channels", label: "Channels", kind: "list", type: "channel" },
    { id: "messages", label: "Messages", kind: "list", type: "message" }
  ],

  events: ["message.captured"],

  agentTools: [
    { name: "slack.sync", schema: {}, impl: "sync", gated: false },
    { name: "slack.read", schema: {}, impl: "list", gated: false },
    { name: "slack.post", schema: {}, impl: "post", gated: true } // GATED
  ],

  integrations: ["slack"]
});
