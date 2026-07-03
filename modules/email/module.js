/**
 * Email Module
 * Gmail via owned Google OAuth (self-hosted Nango). Reads and triage are
 * free; sending is HUMAN-GATED (docs/SECURITY.md §2).
 */
osRegisterModule({
  id: "email",
  name: "Email",
  icon: "Mail",
  color: "var(--neo-blue)",
  num: 6,
  version: "1.0.0",

  entityTypes: {
    email_thread: {
      label: "Thread",
      plural: "Threads",
      icon: "MessagesSquare",
      attrs: {
        gmail_thread_id: { type: "text", required: true }
      },
      display: {
        title: "title"
      }
    },
    email: {
      label: "Email",
      plural: "Emails",
      icon: "Mail",
      attrs: {
        gmail_id: { type: "text", required: true },
        gmail_thread_id: { type: "text", required: true },
        from: { type: "text", required: false },
        to: { type: "text", required: false },
        subject: { type: "text", required: false },
        snippet: { type: "text", required: false },
        label_ids: { type: "text", required: false },
        unread: { type: "boolean", required: false }
      },
      display: {
        title: "subject",
        subtitle: "from",
        badge: "status"
      },
      lifecycle: ["now", "later", "done"]
    },
    contact: {
      label: "Contact",
      plural: "Contacts",
      icon: "User",
      attrs: {
        email: { type: "text", required: true }
      },
      display: {
        title: "title",
        subtitle: "email"
      }
    },
    mail_label: {
      label: "Label",
      plural: "Labels",
      icon: "Tag",
      attrs: {},
      display: {
        title: "title"
      }
    }
  },

  views: [
    { id: "inbox", label: "Inbox", kind: "list", type: "email" },
    { id: "triage", label: "Triage", kind: "board", type: "email" },
    { id: "threads", label: "Threads", kind: "list", type: "email_thread" }
  ],

  events: ["email.received", "email.triaged", "email.drafted", "email.sent"],

  agentTools: [
    { name: "gmail.sync", schema: {}, impl: "sync", gated: false },
    { name: "gmail.read", schema: {}, impl: "list", gated: false },
    { name: "gmail.search", schema: {}, impl: "list", gated: false },
    { name: "gmail.draft", schema: {}, impl: "draft", gated: false },
    { name: "gmail.send", schema: {}, impl: "send", gated: true } // GATED
  ],

  integrations: ["google-mail"]
});
