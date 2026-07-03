/**
 * Reading Module
 * Save/parse articles, capture highlights (issue #61, docs/MODULES.md §3.6).
 * No owned OAuth credential is needed - fetching a public URL requires no
 * account, so both `reading.save` and `reading.highlight` are free.
 */
osRegisterModule({
  id: "reading",
  name: "Reading",
  icon: "BookOpen",
  color: "var(--neo-mint)",
  num: 11,
  version: "1.0.0",

  entityTypes: {
    article: {
      label: "Article",
      plural: "Articles",
      icon: "FileText",
      attrs: {
        url: { type: "text", required: true },
        title: { type: "text", required: false },
        excerpt: { type: "text", required: false },
        summary: { type: "text", required: false },
        read_state: { type: "text", required: false },
        est_minutes: { type: "number", required: false }
      },
      display: {
        title: "title",
        subtitle: "url"
      }
    },
    highlight: {
      label: "Highlight",
      plural: "Highlights",
      icon: "Highlighter",
      attrs: {
        quote: { type: "text", required: true },
        t_offset: { type: "number", required: false },
        color: { type: "text", required: false }
      },
      display: {
        title: "quote"
      }
    },
    source: {
      label: "Source",
      plural: "Sources",
      icon: "Globe",
      attrs: {
        domain: { type: "text", required: true }
      },
      display: {
        title: "domain"
      }
    }
  },

  views: [
    { id: "articles", label: "Articles", kind: "list", type: "article" },
    { id: "highlights", label: "Highlights", kind: "list", type: "highlight" },
    { id: "sources", label: "Sources", kind: "list", type: "source" }
  ],

  events: ["article.saved", "highlight.created"],

  agentTools: [
    { name: "reading.save", schema: {}, impl: "save", gated: false },
    { name: "reading.highlight", schema: {}, impl: "highlight", gated: false }
  ],

  integrations: []
});
