//! Deterministic query classifiers (docs/AI-MEMORY.md §4):
//! - self-RAG gate: skip long-term retrieval on turns that don't need it,
//! - multi-hop detector: only those queries pay for graph expansion.
//!
//! Both are pure functions of the query text - no LLM, no state - so the
//! hot retrieval path stays zero-LLM-call and the behavior is testable.

const SMALLTALK: &[&str] = &[
    "hi", "hello", "hey", "yo", "thanks", "thank you", "ok", "okay", "yes", "no", "cool",
    "nice", "great", "good morning", "good night", "bye", "goodbye", "lol", "hmm", "sure",
];

/// Self-RAG gate: does this turn plausibly need long-term memory?
/// Deliberately permissive - the cost of a false positive is one cheap local
/// query; the cost of a false negative is a confabulated answer.
pub fn needs_memory(query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.chars().count() < 3 {
        return false;
    }
    if SMALLTALK.contains(&q.as_str()) {
        return false;
    }
    // Short pleasantry with punctuation ("thanks!", "ok then.").
    let stripped: String = q.chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect();
    if SMALLTALK.contains(&stripped.trim()) {
        return false;
    }
    true
}

const MULTI_HOP_MARKERS: &[&str] = &[
    "relate", "related", "relationship", "connect", "connected", "connection between",
    "link between", "linked", "led to", "leads to", "caused", "because of", "why did",
    "which of my", "across my", "how does", "how do", "what explains",
];

/// Multi-hop detector: graph expansion only pays off for queries that ask
/// about relations between things ("Does Memory Need Graphs?", 2026);
/// single-hop lookups stay on the cheap FTS5+vector path.
pub fn is_multi_hop(query: &str) -> bool {
    let q = query.to_lowercase();
    MULTI_HOP_MARKERS.iter().any(|m| q.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_skips_smalltalk_and_keeps_real_questions() {
        assert!(!needs_memory("hi"));
        assert!(!needs_memory("Thanks!"));
        assert!(!needs_memory("ok"));
        assert!(!needs_memory(""));
        assert!(needs_memory("what did I decide about the RELIANCE swing trade?"));
        assert!(needs_memory("summarize my week"));
    }

    #[test]
    fn multi_hop_detector_fires_only_on_relational_queries() {
        assert!(is_multi_hop("this drawdown relates to which study topic?"));
        assert!(is_multi_hop("why did the deploy fail after the config change"));
        assert!(!is_multi_hop("when is my dentist appointment"));
        assert!(!is_multi_hop("show my open tasks"));
    }
}
