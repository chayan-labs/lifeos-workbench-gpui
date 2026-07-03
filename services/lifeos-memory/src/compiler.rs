//! Hybrid-deterministic context compiler (issue #116, docs/AI-MEMORY.md §6).
//!
//! Working memory per turn is COMPILED, not paged by an LLM: the budget
//! split, the always-include-last-K-turns window, the template, and the
//! truncation rule are all deterministic; only *which* facts fill the
//! retrieval budget is scored (the activation ranking from retrieval.rs).
//! Same memory state + same query => byte-identical context, within budget.

use crate::retrieval::RecalledMemory;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Turn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy)]
pub struct BudgetSpec {
    pub total_tokens: usize,
    /// Fraction of the budget for recent turns verbatim.
    pub recent_share: f64,
    /// Fraction for retrieved facts (filled by activation order).
    pub facts_share: f64,
    /// Fraction for procedural rules.
    pub rules_share: f64,
    /// Always try to include the last K turns (newest win when over budget).
    pub recent_turns_k: usize,
}

impl Default for BudgetSpec {
    fn default() -> Self {
        Self {
            total_tokens: 2000,
            recent_share: 0.5,
            facts_share: 0.35,
            rules_share: 0.15,
            recent_turns_k: 6,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Section {
    pub name: String,
    pub items: Vec<String>,
    pub tokens: usize,
    /// How many candidates didn't fit (never silently hidden).
    pub dropped: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CompiledContext {
    pub text: String,
    pub sections: Vec<Section>,
    pub tokens_used: usize,
    pub budget_tokens: usize,
}

/// Deterministic token estimate: ceil(chars / 4). Coarse but stable, which
/// is what a reproducible compiler needs; the budget is a working-set bound,
/// not a billing meter.
pub fn est_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

/// Fill a section greedily in the given (already-prioritized) order with
/// whole-item granularity: an item that doesn't fit is dropped, never split,
/// so output is reproducible and each surviving item stays intact.
fn fill_section(name: &str, candidates: &[String], budget: usize) -> Section {
    let mut items = Vec::new();
    let mut tokens = 0;
    let mut dropped = 0;
    for c in candidates {
        let t = est_tokens(c);
        if tokens + t <= budget {
            items.push(c.clone());
            tokens += t;
        } else {
            dropped += 1;
        }
    }
    Section { name: name.to_string(), items, tokens, dropped }
}

/// Compile the working set: procedural rules + retrieved facts (activation
/// order, provenance inline) + last-K recent turns.
pub fn compile_context(
    recent_turns: &[Turn],
    memories: &[RecalledMemory],
    rules: &[String],
    spec: &BudgetSpec,
) -> CompiledContext {
    let rules_budget = (spec.total_tokens as f64 * spec.rules_share) as usize;
    let facts_budget = (spec.total_tokens as f64 * spec.facts_share) as usize;
    let recent_budget = (spec.total_tokens as f64 * spec.recent_share) as usize;

    let rule_items: Vec<String> = rules.iter().map(|r| format!("- {r}")).collect();
    let rules_section = fill_section("procedural_rules", &rule_items, rules_budget);

    // Memories arrive already activation-sorted from retrieval; keep that
    // order and carry provenance inline so nothing recalled is untraceable.
    // The activation VALUE is deliberately not rendered: recall itself bumps
    // access_count (ACT-R), so embedding the score would make two otherwise
    // identical compilations differ - the score breakdown lives in the
    // recall/ledger payload instead.
    let fact_items: Vec<String> = memories
        .iter()
        .map(|m| format!("- (src={}) {}", m.source_event_ids.join(","), m.content))
        .collect();
    let facts_section = fill_section("retrieved_memories", &fact_items, facts_budget);

    // Last K turns, newest guaranteed first pick, rendered chronologically.
    let window: Vec<&Turn> = recent_turns
        .iter()
        .rev()
        .take(spec.recent_turns_k)
        .collect();
    let mut kept: Vec<String> = Vec::new();
    let mut recent_tokens = 0;
    let mut recent_dropped = recent_turns.len().saturating_sub(spec.recent_turns_k);
    for turn in &window {
        let line = format!("{}: {}", turn.role, turn.content);
        let t = est_tokens(&line);
        if recent_tokens + t <= recent_budget {
            kept.push(line);
            recent_tokens += t;
        } else {
            recent_dropped += 1;
        }
    }
    kept.reverse(); // back to chronological order
    let recent_section = Section {
        name: "recent_turns".to_string(),
        items: kept,
        tokens: recent_tokens,
        dropped: recent_dropped,
    };

    let sections = vec![rules_section, facts_section, recent_section];
    let mut text = String::new();
    for s in &sections {
        if s.items.is_empty() {
            continue;
        }
        text.push_str(&format!("## {}\n", s.name));
        for item in &s.items {
            text.push_str(item);
            text.push('\n');
        }
        text.push('\n');
    }
    let tokens_used = sections.iter().map(|s| s.tokens).sum();
    CompiledContext { text, sections, tokens_used, budget_tokens: spec.total_tokens }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::ActivationBreakdown;

    fn mem(id: &str, content: &str, activation: f64) -> RecalledMemory {
        RecalledMemory {
            id: id.into(),
            kind: "episodic".into(),
            content: content.into(),
            ts: 0,
            confidence: 1.0,
            access_count: 0,
            source_event_ids: vec![format!("evt_{id}")],
            tiered_ref: None,
            breakdown: ActivationBreakdown {
                relevance: activation,
                recency: 1.0,
                importance: 1.0,
                frequency: 1.0,
                activation,
                via_graph_hops: None,
            },
        }
    }

    #[test]
    fn compilation_is_reproducible_and_within_budget() {
        let turns = vec![
            Turn { role: "user".into(), content: "how did the trade go?".into() },
            Turn { role: "assistant".into(), content: "booked profit".into() },
        ];
        let memories = vec![mem("a", "closed RELIANCE +4200", 0.9), mem("b", "opened swing", 0.4)];
        let rules = vec!["lead with the TLDR".to_string()];
        let spec = BudgetSpec::default();

        let one = compile_context(&turns, &memories, &rules, &spec);
        let two = compile_context(&turns, &memories, &rules, &spec);
        assert_eq!(one.text, two.text, "same inputs => byte-identical context");
        assert!(one.tokens_used <= spec.total_tokens);
        assert!(one.text.contains("src=evt_a"), "provenance travels into the context");
    }

    #[test]
    fn over_budget_drops_lowest_activation_facts_and_keeps_newest_turns() {
        let turns: Vec<Turn> = (0..20)
            .map(|i| Turn { role: "user".into(), content: format!("turn number {i} {}", "x".repeat(80)) })
            .collect();
        let memories: Vec<RecalledMemory> = (0..30)
            .map(|i| mem(&format!("m{i:02}"), &format!("fact {i} {}", "y".repeat(100)), 1.0 - i as f64 / 30.0))
            .collect();
        let spec = BudgetSpec { total_tokens: 300, ..Default::default() };
        let ctx = compile_context(&turns, &memories, &[], &spec);

        assert!(ctx.tokens_used <= 300);
        let facts = &ctx.sections[1];
        assert!(facts.dropped > 0, "over-budget candidates are reported, not hidden");
        // Highest-activation fact survives; the tail is what got dropped.
        assert!(facts.items.first().unwrap().contains("fact 0"));
        // Newest turn survives the recency window.
        let recent = &ctx.sections[2];
        assert!(recent.items.iter().any(|l| l.contains("turn number 19")));
        assert!(recent.dropped > 0);
    }
}
