//! `GET /api/pipeline/registry` (issue #94) - lets the frontend inspect the
//! DAG stages of every registered pipeline instead of hardcoding them
//! (`Dashboard.jsx`'s old `PIPELINE_STAGE_ORDER`/`_META`). Reads
//! `lifeos_pipelines::pipeline_registry()` directly - it's a static Rust
//! table, not tenant data, so this route needs no workspace scoping or DB
//! access, same as any other constant-configuration endpoint.

use axum::Json;
use serde_json::{json, Value};

pub async fn registry() -> Json<Value> {
    let pipelines: Vec<Value> = lifeos_pipelines::pipeline_registry()
        .values()
        .map(|spec| {
            json!({
                "id": spec.id,
                "stages": spec.stages.iter().map(|s| json!({
                    "name": s.name,
                    "agent": s.agent,
                    "tool": s.tool,
                    "skill": s.skill,
                    "gate": s.gate,
                    "gated": s.gated,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Json(json!(pipelines))
}
