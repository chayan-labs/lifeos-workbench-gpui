/**
 * Learning / Study Module
 * Handles knowledge domains, topics, subtopics, spaced repetition, examiner teach-back.
 */
osRegisterModule({
  id: "learning",
  name: "Learning & Study",
  icon: "BookOpen",
  color: "var(--neo-mint)",
  num: 1,
  version: "1.0.0",

  entityTypes: {
    domain: {
      label: "Domain",
      plural: "Domains",
      icon: "Folder",
      attrs: {
        name: { type: "text", required: true }
      },
      display: { title: "name" }
    },
    topic: {
      label: "Topic",
      plural: "Topics",
      icon: "FileText",
      attrs: {
        summary: { type: "text", required: true },
        mastery: { type: "number", required: true }, // 0.0 - 1.0
        last_review: { type: "date", required: false },
        next_due: { type: "date", required: false },
        difficulty: { type: "number", required: false }
      },
      display: {
        title: "summary",
        badge: "mastery"
      },
      lifecycle: ["learning", "mastered", "review_due"]
    }
  },

  views: [
    { id: "all_topics", label: "Knowledge Tree", kind: "graph", type: "topic" },
    { id: "review_due", label: "Spaced Repetition", kind: "calendar", type: "topic", filter: "status = 'review_due'" }
  ],

  events: ["study.review", "topic.added", "gap.opened", "quiz.answered"],
  
  botCommands: [
    { cmd: "addtopic", help: "Capture a new topic to study", handler: "add_topic" },
    { cmd: "quizme", help: "Run a teach-back session on a topic", handler: "quiz" }
  ],

  agentTools: [
    { name: "learn.add_topic", schema: {}, impl: "add_topic", gated: false },
    { name: "learn.quiz", schema: {}, impl: "quiz", gated: false }
  ]
});
