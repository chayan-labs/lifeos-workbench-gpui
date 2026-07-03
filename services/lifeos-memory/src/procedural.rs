//! Procedural memory (issue #118, docs/AI-MEMORY.md §8): learned behavioral
//! rules ("lead with the TLDR", "include market regime on trade questions")
//! that feed the system prompt.
//!
//! The store itself is just the `memory_rules` read model - materialized by
//! the projector from memory.rule.added / memory.rule.retired events, so
//! rules are event-sourced like everything else and never edited in place.
//! Rule LEARNING is behind the `PolicyLearner` seam (inputs: candidate
//! events, outputs: rule deltas) so the heuristic below can be swapped for a
//! learned policy (Memory-R1) with no architectural change. Consolidation
//! calls the learner off the hot path.

use crate::error::MemoryError;
use crate::project::EventRecord;
use libsql::{params, Connection};

#[derive(Debug, Clone, PartialEq)]
pub enum RuleDelta {
    Add { rule: String, confidence: f64, source_event_ids: Vec<String> },
    Retire { rule_id: String, source_event_id: String },
}

pub trait PolicyLearner: Send + Sync {
    fn derive_rule_deltas(&self, events: &[EventRecord]) -> Vec<RuleDelta>;
}

/// Deterministic baseline learner: explicit user feedback becomes a rule.
/// - `feedback.given` events (or any event with an attrs.feedback string)
///   add a rule from the feedback text.
/// - attrs.retract_rule_id retires one.
pub struct HeuristicPolicyLearner;

impl PolicyLearner for HeuristicPolicyLearner {
    fn derive_rule_deltas(&self, events: &[EventRecord]) -> Vec<RuleDelta> {
        let mut deltas = Vec::new();
        for ev in events {
            if let Some(rid) = ev.attrs.get("retract_rule_id").and_then(|v| v.as_str()) {
                deltas.push(RuleDelta::Retire {
                    rule_id: rid.to_string(),
                    source_event_id: ev.id.clone(),
                });
                continue;
            }
            let feedback = ev.attrs.get("feedback").and_then(|v| v.as_str());
            let is_feedback_event = ev.event_type == "feedback.given";
            if let Some(text) = feedback.filter(|t| !t.trim().is_empty()) {
                deltas.push(RuleDelta::Add {
                    rule: text.trim().to_string(),
                    confidence: if is_feedback_event { 0.7 } else { 0.5 },
                    source_event_ids: vec![ev.id.clone()],
                });
            }
        }
        deltas
    }
}

/// Compile the active rules into a system-prompt block, highest confidence
/// first (id as deterministic tiebreak), bounded by a token budget.
pub async fn rules_for_prompt(
    conn: &Connection,
    workspace_id: &str,
    max_tokens: usize,
) -> Result<Vec<String>, MemoryError> {
    let mut rows = conn
        .query(
            "SELECT rule FROM memory_rules \
             WHERE workspace_id = ?1 AND status = 'active' \
             ORDER BY confidence DESC, id ASC",
            params![workspace_id],
        )
        .await?;
    let mut out = Vec::new();
    let mut used = 0;
    while let Some(row) = rows.next().await? {
        let rule: String = row.get(0)?;
        let t = crate::compiler::est_tokens(&rule);
        if used + t > max_tokens {
            break;
        }
        used += t;
        out.push(rule);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::project_workspace;
    use crate::testutil::{seed_event, test_conn};
    use serde_json::json;

    fn ev(id: &str, event_type: &str, attrs: serde_json::Value) -> EventRecord {
        EventRecord {
            id: id.into(),
            ts: 1,
            event_type: event_type.into(),
            entity_id: None,
            actor: Some("user".into()),
            attrs,
            caused_by_event_id: None,
            schema_version: 1,
        }
    }

    #[test]
    fn learner_turns_feedback_into_rule_deltas() {
        let learner = HeuristicPolicyLearner;
        let events = vec![
            ev("e1", "feedback.given", json!({"feedback": "always lead with the TLDR"})),
            ev("e2", "task.completed", json!({"note": "no feedback here"})),
            ev("e3", "feedback.given", json!({"retract_rule_id": "mr_x"})),
        ];
        let deltas = learner.derive_rule_deltas(&events);
        assert_eq!(deltas.len(), 2);
        assert!(matches!(&deltas[0], RuleDelta::Add { rule, .. } if rule == "always lead with the TLDR"));
        assert!(matches!(&deltas[1], RuleDelta::Retire { rule_id, .. } if rule_id == "mr_x"));
    }

    #[tokio::test]
    async fn rules_persist_via_events_and_feed_the_prompt() {
        let conn = test_conn().await;
        seed_event(
            &conn, "ws_1", "evt_r1", 100, "memory.rule.added", None, "harness",
            json!({"rule": "lead with the TLDR", "confidence": 0.9}), None,
        )
        .await;
        seed_event(
            &conn, "ws_1", "evt_r2", 200, "memory.rule.added", None, "harness",
            json!({"rule": "include market regime on trade questions", "confidence": 0.6}), None,
        )
        .await;
        project_workspace(&conn, "ws_1").await.unwrap();

        let rules = rules_for_prompt(&conn, "ws_1", 1000).await.unwrap();
        assert_eq!(rules[0], "lead with the TLDR", "highest confidence first");
        assert_eq!(rules.len(), 2);

        // Budget bound: a tiny budget keeps only the top rule.
        let tight = rules_for_prompt(&conn, "ws_1", 5).await.unwrap();
        assert_eq!(tight.len(), 1);

        // Workspace isolation.
        assert!(rules_for_prompt(&conn, "ws_2", 1000).await.unwrap().is_empty());
    }
}
