//! Spreading-activation graph expansion (issue #114, docs/AI-MEMORY.md §4).
//!
//! Cross-domain recall ("this drawdown relates to which study topic?")
//! emerges from walking `memory_edges` - something flat vector RAG cannot do.
//! Expansion is TIERED: retrieval only calls this for multi-hop queries (see
//! gate::is_multi_hop); everything else stays on the cheap lexical path.
//!
//! Only currently-true edges (`t_invalid IS NULL`) are walked, and edges are
//! treated as undirected for activation purposes (a memory 'about' an entity
//! activates sibling memories about the same entity).

use crate::error::MemoryError;
use libsql::{params, Connection};
use petgraph::graph::{NodeIndex, UnGraph};
use std::collections::HashMap;

/// Expand up to `max_hops` (clamped to 2) from the seed memory ids. Returns
/// `(memory_node_id, hops)` for newly reached MEMORY nodes (entity vertices
/// participate in the walk but are not returned - they aren't recallable
/// content). Deterministic: neighbors discovered in sorted edge order.
pub async fn expand_seeds(
    conn: &Connection,
    workspace_id: &str,
    seeds: &[String],
    max_hops: usize,
) -> Result<Vec<(String, usize)>, MemoryError> {
    let max_hops = max_hops.min(2);
    if seeds.is_empty() || max_hops == 0 {
        return Ok(Vec::new());
    }

    // Load the current adjacency for this workspace into petgraph.
    let mut rows = conn
        .query(
            "SELECT from_id, to_id FROM memory_edges \
             WHERE workspace_id = ?1 AND t_invalid IS NULL \
             ORDER BY id",
            params![workspace_id],
        )
        .await?;
    let mut graph: UnGraph<String, ()> = UnGraph::new_undirected();
    let mut index: HashMap<String, NodeIndex> = HashMap::new();
    let intern = |graph: &mut UnGraph<String, ()>,
                      index: &mut HashMap<String, NodeIndex>,
                      id: &str| {
        *index
            .entry(id.to_string())
            .or_insert_with(|| graph.add_node(id.to_string()))
    };
    while let Some(row) = rows.next().await? {
        let from: String = row.get(0)?;
        let to: String = row.get(1)?;
        let a = intern(&mut graph, &mut index, &from);
        let b = intern(&mut graph, &mut index, &to);
        graph.update_edge(a, b, ());
    }

    // BFS out to max_hops from all seeds at once (multi-source).
    let mut hops: HashMap<NodeIndex, usize> = HashMap::new();
    let mut frontier: Vec<NodeIndex> = Vec::new();
    for seed in seeds {
        if let Some(&ix) = index.get(seed) {
            hops.insert(ix, 0);
            frontier.push(ix);
        }
    }
    for depth in 1..=max_hops {
        let mut next = Vec::new();
        for &node in &frontier {
            let mut neighbors: Vec<NodeIndex> = graph.neighbors(node).collect();
            neighbors.sort();
            for n in neighbors {
                if let std::collections::hash_map::Entry::Vacant(e) = hops.entry(n) {
                    e.insert(depth);
                    next.push(n);
                }
            }
        }
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }

    let mut out: Vec<(String, usize)> = hops
        .into_iter()
        .filter(|(_, d)| *d > 0)
        .map(|(ix, d)| (graph[ix].clone(), d))
        // Only memory nodes are recallable; entity vertices are bridges.
        .filter(|(id, _)| id.starts_with("mn_"))
        .collect();
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::is_multi_hop;
    use crate::project::project_workspace;
    use crate::retrieval::{recall, NoopVectorSearcher, RecallOutcome, RecallParams};
    use crate::testutil::{seed_entity, seed_event, test_conn_with_derived};
    use serde_json::json;

    /// The issue-#114 acceptance query: a drawdown event and a study-topic
    /// event share an entity; a multi-hop query about the drawdown surfaces
    /// the linked study memory via 2-hop expansion.
    #[tokio::test]
    async fn multi_hop_query_surfaces_linked_memory_and_single_hop_skips() {
        let conn = test_conn_with_derived().await;
        seed_entity(&conn, "ws_1", "ent_topic", "learning", "Market microstructure").await;
        seed_event(
            &conn, "ws_1", "evt_drawdown", 1000, "trade.closed", Some("ent_topic"), "user",
            json!({"pnl": -8000, "note": "drawdown on the banknifty position"}), None,
        )
        .await;
        // Deliberately shares NO token with the query below, so it can only
        // surface via the graph walk, never via a direct lexical hit.
        seed_event(
            &conn, "ws_1", "evt_study", 2000, "learning.review", Some("ent_topic"), "user",
            json!({"summary": "reviewed liquidity sweeps and stop hunts"}), None,
        )
        .await;
        project_workspace(&conn, "ws_1").await.unwrap();

        let query = "this drawdown relates to which subject?";
        assert!(is_multi_hop(query));
        let out = recall(
            &conn, "ws_1", query, 3000,
            &RecallParams { abstention_threshold: 0.0, ..Default::default() },
            &NoopVectorSearcher,
        )
        .await
        .unwrap();
        let RecallOutcome::Recalled { memories, expanded_graph } = out else { panic!() };
        assert!(expanded_graph);
        let via_graph = memories
            .iter()
            .find(|m| m.content.contains("liquidity sweeps"))
            .expect("2-hop neighbor (drawdown -> entity -> study memory) must surface");
        assert_eq!(via_graph.breakdown.via_graph_hops, Some(2));

        // Single-hop phrasing: no expansion.
        let out = recall(
            &conn, "ws_1", "banknifty drawdown", 3000,
            &RecallParams { abstention_threshold: 0.0, ..Default::default() },
            &NoopVectorSearcher,
        )
        .await
        .unwrap();
        let RecallOutcome::Recalled { expanded_graph, .. } = out else { panic!() };
        assert!(!expanded_graph, "single-hop queries stay on the cheap path");
    }

    #[tokio::test]
    async fn expansion_ignores_invalidated_edges() {
        let conn = test_conn_with_derived().await;
        seed_entity(&conn, "ws_1", "ent_x", "misc", "Shared thing").await;
        seed_event(
            &conn, "ws_1", "evt_a", 100, "note.captured", Some("ent_x"), "user",
            json!({"text": "alpha"}), None,
        )
        .await;
        seed_event(
            &conn, "ws_1", "evt_b", 200, "note.captured", Some("ent_x"), "user",
            json!({"text": "beta"}), None,
        )
        .await;
        // Supersede evt_a: its node AND edges become invalid.
        seed_event(
            &conn, "ws_1", "evt_sup", 300, "memory.supersede.detected", None, "harness",
            json!({"old_event_id": "evt_a"}), None,
        )
        .await;
        project_workspace(&conn, "ws_1").await.unwrap();

        let seed = crate::project::node_id("ws_1", "evt_b");
        let reached = expand_seeds(&conn, "ws_1", &[seed], 2).await.unwrap();
        let ids: Vec<&String> = reached.iter().map(|(id, _)| id).collect();
        let dead = crate::project::node_id("ws_1", "evt_a");
        assert!(!ids.contains(&&dead), "invalidated edges must not conduct activation");
    }
}
