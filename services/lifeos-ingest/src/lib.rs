//! lifeos-ingest: the media→text orchestrator (issue #88, docs/MEDIA-INTELLIGENCE.md).
//!
//! `lifeos-drain` claims `ingest` jobs from `jobs` and calls `process_ingest_job`
//! directly as a library (no subprocess - both crates are Rust in the same
//! workspace). This crate has no dependency on `lifeos-api` (same standalone
//! style as `lifeos-drain`): it reads/writes `entities`/`events` with its own
//! small SQL, mirroring `lifeos-drain`'s `emit_event` mirror of
//! `lifeos_api::audit::emit`.
//!
//! Scope of #88 is the orchestrator, not the extractors: MIME routing, job
//! dispatch, segment-entity creation, and triggering memvec indexing. Real
//! extraction lands only for plain text here - audio/video transcription
//! (#89) and image/PDF/docx captioning/OCR/text-extraction (#90) are honestly
//! reported as `Unsupported { kind, blocked_by }`, matching the same
//! `blocked_by` issue numbers `lifeos_vcs::diff::blocking_issue_for` already
//! uses for those same MIME classes.

use async_trait::async_trait;
use libsql::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fmt;
use std::sync::Mutex;
use ulid::{Generator, Ulid};

static ID_GENERATOR: Mutex<Generator> = Mutex::new(Generator::new());

fn new_id(prefix: &str) -> String {
    let ulid = ID_GENERATOR
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .generate()
        .unwrap_or_else(|_| Ulid::new());
    format!("{prefix}_{ulid}")
}

/// Mirrors `lifeos_api::audit::emit` / `lifeos_drain::emit_event`'s shape
/// exactly (same table, same id scheme) so events this crate writes are
/// indistinguishable from ones the API writes.
async fn emit_event(
    conn: &Connection,
    workspace_id: &str,
    event_type: &str,
    entity_id: &str,
    attrs: &Value,
    now: i64,
) -> libsql::Result<()> {
    let attrs_str = serde_json::to_string(attrs).unwrap_or_else(|_| "{}".into());
    conn.execute(
        "INSERT INTO events (id, workspace_id, ts, type, entity_id, actor, attrs) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'lifeos-ingest', ?6)",
        params![new_id("evt"), workspace_id, now, event_type, entity_id, attrs_str],
    )
    .await?;
    Ok(())
}

/// A claimed `ingest` job's payload (`routes/planned.rs::IngestRequest`'s
/// enqueued shape).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct IngestJobPayload {
    pub entity_id: Option<String>,
    pub uri: Option<String>,
    pub kind: Option<String>,
    pub blob_ref: Option<String>,
}

/// MIME routing decision. Only `PlainText` has a real extractor today; every
/// other kind is a named, honest gap - never a silent no-op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MimeRoute {
    PlainText,
    Unsupported { kind: &'static str, blocked_by: &'static str },
}

/// Routes a file name (falling back to a job's `kind` hint) to its MIME
/// class. Same issue numbers `lifeos_vcs::diff::blocking_issue_for` already
/// names for these MIME classes, so both subsystems agree on what's blocked.
pub fn route_by_mime(name: &str, hint_kind: Option<&str>) -> MimeRoute {
    let lower = name.to_ascii_lowercase();
    const TEXT_EXT: &[&str] = &[".txt", ".md", ".markdown", ".csv", ".log", ".json"];
    const AUDIO_EXT: &[&str] = &[".mp3", ".wav", ".m4a"];
    const VIDEO_EXT: &[&str] = &[".mp4", ".mov", ".webm"];
    const IMAGE_EXT: &[&str] = &[".png", ".jpg", ".jpeg", ".gif", ".webp"];

    if TEXT_EXT.iter().any(|ext| lower.ends_with(ext)) {
        return MimeRoute::PlainText;
    }
    if AUDIO_EXT.iter().any(|ext| lower.ends_with(ext)) {
        return MimeRoute::Unsupported { kind: "audio", blocked_by: "lifeos-ingest transcription (#89)" };
    }
    if VIDEO_EXT.iter().any(|ext| lower.ends_with(ext)) {
        return MimeRoute::Unsupported { kind: "video", blocked_by: "lifeos-ingest transcription (#89)" };
    }
    if IMAGE_EXT.iter().any(|ext| lower.ends_with(ext)) {
        return MimeRoute::Unsupported { kind: "image", blocked_by: "lifeos-ingest image captioning (#90)" };
    }
    if lower.ends_with(".pdf") {
        return MimeRoute::Unsupported { kind: "pdf", blocked_by: "lifeos-ingest text extraction (#90)" };
    }
    if lower.ends_with(".docx") {
        return MimeRoute::Unsupported { kind: "docx", blocked_by: "lifeos-ingest text extraction (#90)" };
    }
    match hint_kind {
        Some("audio") => MimeRoute::Unsupported { kind: "audio", blocked_by: "lifeos-ingest transcription (#89)" },
        Some("video") => MimeRoute::Unsupported { kind: "video", blocked_by: "lifeos-ingest transcription (#89)" },
        Some("image") => MimeRoute::Unsupported { kind: "image", blocked_by: "lifeos-ingest image captioning (#90)" },
        _ => MimeRoute::Unsupported { kind: "unknown", blocked_by: "no MIME route defined for this file type yet" },
    }
}

/// Splits plain text into blank-line-separated paragraphs. Honest, no
/// fabricated NLP segmentation - a chunk is exactly what it looks like.
pub fn chunk_plain_text(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// Fire-and-forget trigger for memvec indexing. A failed embed never blocks
/// the DB state transition - the boot `rebuild` reconciles any drift, same
/// reasoning as `lifeos_api::db::index_entity`'s doc comment.
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, workspace_id: &str, entity_id: &str, text: &str);
}

/// Used when no memvec is configured (`LIFEOS_MEMVEC` unset) - segments are
/// still created and lexically indexable, just not semantically searchable.
pub struct NoopEmbedder;

#[async_trait]
impl Embedder for NoopEmbedder {
    async fn embed(&self, _workspace_id: &str, entity_id: &str, _text: &str) {
        eprintln!("lifeos-ingest: no embedder configured, skipping semantic index for {entity_id}");
    }
}

/// Shells out to `server/memvec.py embed`, the same subprocess shape
/// `routes/search.rs`'s query call already uses.
pub struct SubprocessEmbedder {
    pub memvec_path: String,
    pub derived_db_path: String,
}

#[async_trait]
impl Embedder for SubprocessEmbedder {
    async fn embed(&self, workspace_id: &str, entity_id: &str, text: &str) {
        let result = tokio::process::Command::new("python3")
            .arg(&self.memvec_path)
            .arg("--db")
            .arg(&self.derived_db_path)
            .arg("embed")
            .arg("--workspace")
            .arg(workspace_id)
            .arg("--id")
            .arg(entity_id)
            .arg("--text")
            .arg(text)
            .output()
            .await;
        match result {
            Ok(out) if out.status.success() => {}
            Ok(out) => eprintln!(
                "lifeos-ingest: memvec embed failed for {entity_id}: {}",
                String::from_utf8_lossy(&out.stderr)
            ),
            Err(e) => eprintln!("lifeos-ingest: memvec embed spawn failed for {entity_id}: {e}"),
        }
    }
}

/// A completed (or honestly-degraded) ingest run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestOutcome {
    Completed { segment_ids: Vec<String> },
    Unsupported { kind: String, blocked_by: String },
}

/// A real failure - bad input, not a MIME gap. `lifeos-drain` fails the job
/// (no retry-forever loop) rather than completing it.
#[derive(Debug)]
pub enum IngestError {
    MissingEntityId,
    EntityNotFound(String),
    NoContent,
    Db(libsql::Error),
    Io(std::io::Error),
}

impl fmt::Display for IngestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IngestError::MissingEntityId => write!(f, "ingest job payload has no entity_id"),
            IngestError::EntityNotFound(id) => write!(f, "entity '{id}' not found in this workspace"),
            IngestError::NoContent => write!(f, "no blob_ref available (neither payload nor entity has one)"),
            IngestError::Db(e) => write!(f, "db error: {e}"),
            IngestError::Io(e) => write!(f, "blob store error: {e}"),
        }
    }
}

struct SourceEntity {
    module: String,
    title: Option<String>,
    attrs: Value,
    blob_ref: Option<String>,
}

async fn fetch_entity(conn: &Connection, workspace_id: &str, id: &str) -> Result<SourceEntity, IngestError> {
    let mut rows = conn
        .query(
            "SELECT module, title, attrs, blob_ref FROM entities WHERE id=?1 AND workspace_id=?2",
            params![id, workspace_id],
        )
        .await
        .map_err(IngestError::Db)?;
    match rows.next().await.map_err(IngestError::Db)? {
        Some(row) => {
            let module: String = row.get(0).map_err(IngestError::Db)?;
            let title: Option<String> = row.get(1).map_err(IngestError::Db)?;
            let attrs_str: String = row.get(2).map_err(IngestError::Db)?;
            let blob_ref: Option<String> = row.get(3).map_err(IngestError::Db)?;
            let attrs: Value = serde_json::from_str(&attrs_str).unwrap_or_else(|_| json!({}));
            Ok(SourceEntity { module, title, attrs, blob_ref })
        }
        None => Err(IngestError::EntityNotFound(id.to_string())),
    }
}

fn entity_display_name(entity: &SourceEntity, payload: &IngestJobPayload) -> String {
    if let Some(name) = entity.attrs.get("name").and_then(|v| v.as_str()) {
        return name.to_string();
    }
    if let Some(title) = &entity.title {
        return title.clone();
    }
    if let Some(uri) = &payload.uri {
        return uri.rsplit('/').next().unwrap_or(uri).to_string();
    }
    String::new()
}

/// The core, fully-testable orchestration `lifeos-drain` calls for every
/// claimed `ingest` job.
pub async fn process_ingest_job(
    conn: &Connection,
    store: &lifeos_vcs::ObjectStore,
    embedder: &(dyn Embedder + Sync),
    workspace_id: &str,
    payload: IngestJobPayload,
    now: i64,
) -> Result<IngestOutcome, IngestError> {
    let entity_id = payload.entity_id.clone().ok_or(IngestError::MissingEntityId)?;
    let entity = fetch_entity(conn, workspace_id, &entity_id).await?;

    let blob_ref = payload
        .blob_ref
        .clone()
        .or_else(|| entity.blob_ref.clone())
        .ok_or(IngestError::NoContent)?;

    let name = entity_display_name(&entity, &payload);
    let route = route_by_mime(&name, payload.kind.as_deref());

    match route {
        MimeRoute::PlainText => {
            let bytes = lifeos_vcs::read_blob(store, &blob_ref).map_err(IngestError::Io)?;
            let text = match String::from_utf8(bytes) {
                Ok(t) => t,
                Err(_) => {
                    return finish_unsupported(conn, workspace_id, &entity_id, "binary", "no MIME route defined for this file type yet", now)
                        .await;
                }
            };

            let chunks = chunk_plain_text(&text);
            let mut segment_ids = Vec::with_capacity(chunks.len());
            for chunk in &chunks {
                let seg_id = new_id("segment");
                let attrs = json!({ "text": chunk });
                conn.execute(
                    "INSERT INTO entities (id, workspace_id, module, type, parent_id, attrs, source, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, 'segment', ?4, ?5, 'lifeos-ingest', ?6, ?6)",
                    params![
                        seg_id.clone(),
                        workspace_id,
                        entity.module.clone(),
                        entity_id.clone(),
                        serde_json::to_string(&attrs).unwrap_or_else(|_| "{}".into()),
                        now
                    ],
                )
                .await
                .map_err(IngestError::Db)?;
                embedder.embed(workspace_id, &seg_id, chunk).await;
                segment_ids.push(seg_id);
            }

            let transcript_ref = lifeos_vcs::store_blob(store, text.as_bytes()).map_err(IngestError::Io)?;
            conn.execute(
                "UPDATE entities SET attrs = json_set(attrs, '$.transcript_ref', ?1) WHERE id=?2",
                params![transcript_ref, entity_id.clone()],
            )
            .await
            .map_err(IngestError::Db)?;

            emit_event(
                conn,
                workspace_id,
                "ingest.completed",
                &entity_id,
                &json!({ "segment_count": segment_ids.len() }),
                now,
            )
            .await
            .map_err(IngestError::Db)?;

            Ok(IngestOutcome::Completed { segment_ids })
        }
        MimeRoute::Unsupported { kind, blocked_by } => {
            finish_unsupported(conn, workspace_id, &entity_id, kind, blocked_by, now).await
        }
    }
}

/// Known, correctly-handled gap - not a failure. The job completes (nothing
/// to retry until #89/#90 land) and the parent entity records why, in the
/// same `{kind, blocked_by}` shape `/api/vcs/diff` already uses.
async fn finish_unsupported(
    conn: &Connection,
    workspace_id: &str,
    entity_id: &str,
    kind: &str,
    blocked_by: &str,
    now: i64,
) -> Result<IngestOutcome, IngestError> {
    conn.execute(
        "UPDATE entities SET attrs = json_set(attrs, '$.ingest_status', 'unsupported', '$.ingest_blocked_by', ?1) WHERE id=?2",
        params![blocked_by, entity_id],
    )
    .await
    .map_err(IngestError::Db)?;

    emit_event(
        conn,
        workspace_id,
        "ingest.unsupported",
        entity_id,
        &json!({ "kind": kind, "blocked_by": blocked_by }),
        now,
    )
    .await
    .map_err(IngestError::Db)?;

    Ok(IngestOutcome::Unsupported { kind: kind.to_string(), blocked_by: blocked_by.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use libsql::Builder;
    use lifeos_vcs::ObjectStore;

    async fn fresh_conn(path: &str) -> Connection {
        let _ = std::fs::remove_file(path);
        let db = Builder::new_local(path).build().await.unwrap();
        let conn = db.connect().unwrap();
        conn.execute(
            "CREATE TABLE entities (\
                id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, module TEXT, type TEXT, \
                parent_id TEXT, title TEXT, attrs TEXT NOT NULL DEFAULT '{}', \
                source TEXT, blob_ref TEXT, created_at INTEGER, updated_at INTEGER)",
            (),
        )
        .await
        .unwrap();
        conn.execute(
            "CREATE TABLE events (\
                id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, ts INTEGER, type TEXT, \
                entity_id TEXT, actor TEXT, attrs TEXT)",
            (),
        )
        .await
        .unwrap();
        conn
    }

    async fn insert_file_entity(conn: &Connection, id: &str, workspace_id: &str, name: &str, blob_ref: &str) {
        let attrs = json!({ "name": name });
        conn.execute(
            "INSERT INTO entities (id, workspace_id, module, type, attrs, blob_ref, created_at, updated_at) \
             VALUES (?1, ?2, 'files', 'file', ?3, ?4, 0, 0)",
            params![id, workspace_id, serde_json::to_string(&attrs).unwrap(), blob_ref],
        )
        .await
        .unwrap();
    }

    struct MockEmbedder {
        calls: Mutex<Vec<(String, String)>>,
    }

    impl MockEmbedder {
        fn new() -> Self {
            Self { calls: Mutex::new(Vec::new()) }
        }
    }

    #[async_trait]
    impl Embedder for MockEmbedder {
        async fn embed(&self, _workspace_id: &str, entity_id: &str, text: &str) {
            self.calls.lock().unwrap().push((entity_id.to_string(), text.to_string()));
        }
    }

    #[test]
    fn route_by_mime_dispatches_by_extension() {
        assert_eq!(route_by_mime("notes.txt", None), MimeRoute::PlainText);
        assert_eq!(route_by_mime("README.md", None), MimeRoute::PlainText);
        match route_by_mime("clip.mp4", None) {
            MimeRoute::Unsupported { kind, blocked_by } => {
                assert_eq!(kind, "video");
                assert!(blocked_by.contains("#89"));
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
        match route_by_mime("cover.png", None) {
            MimeRoute::Unsupported { kind, blocked_by } => {
                assert_eq!(kind, "image");
                assert!(blocked_by.contains("#90"));
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
        match route_by_mime("weird.xyz", None) {
            MimeRoute::Unsupported { kind, blocked_by } => {
                assert_eq!(kind, "unknown");
                assert!(!blocked_by.is_empty());
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn chunk_plain_text_splits_on_blank_lines_and_drops_empties() {
        let chunks = chunk_plain_text("first para\n\n\n  second para  \n\n");
        assert_eq!(chunks, vec!["first para".to_string(), "second para".to_string()]);
        assert!(chunk_plain_text("   \n\n  ").is_empty());
    }

    #[tokio::test]
    async fn plain_text_produces_segments_transcript_ref_and_embeds() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let conn = fresh_conn("test_ingest_plain.db").await;

        let blob_ref = lifeos_vcs::store_blob(&store, b"para one\n\npara two").unwrap();
        insert_file_entity(&conn, "ent_file", "ws_1", "notes.txt", &blob_ref).await;

        let embedder = MockEmbedder::new();
        let payload = IngestJobPayload { entity_id: Some("ent_file".into()), ..Default::default() };
        let outcome = process_ingest_job(&conn, &store, &embedder, "ws_1", payload, 100).await.unwrap();

        let segment_ids = match outcome {
            IngestOutcome::Completed { segment_ids } => segment_ids,
            other => panic!("expected Completed, got {other:?}"),
        };
        assert_eq!(segment_ids.len(), 2);

        let mut rows = conn
            .query(
                "SELECT parent_id, type, json_extract(attrs,'$.text') FROM entities WHERE type='segment' ORDER BY id",
                (),
            )
            .await
            .unwrap();
        let mut seen = 0;
        while let Some(row) = rows.next().await.unwrap() {
            let parent_id: String = row.get(0).unwrap();
            let kind: String = row.get(1).unwrap();
            let text: String = row.get(2).unwrap();
            assert_eq!(parent_id, "ent_file");
            assert_eq!(kind, "segment");
            assert!(!text.is_empty());
            seen += 1;
        }
        assert_eq!(seen, 2);

        assert_eq!(embedder.calls.lock().unwrap().len(), 2);

        let mut parent_rows = conn
            .query("SELECT json_extract(attrs,'$.transcript_ref') FROM entities WHERE id='ent_file'", ())
            .await
            .unwrap();
        let transcript_ref: Option<String> = parent_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert!(transcript_ref.is_some());

        let mut evt_rows = conn.query("SELECT type FROM events WHERE entity_id='ent_file'", ()).await.unwrap();
        let evt_type: String = evt_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(evt_type, "ingest.completed");
    }

    #[tokio::test]
    async fn unsupported_mime_creates_no_segments_and_marks_the_parent() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let conn = fresh_conn("test_ingest_unsupported.db").await;

        let blob_ref = lifeos_vcs::store_blob(&store, b"fake video bytes").unwrap();
        insert_file_entity(&conn, "ent_clip", "ws_1", "clip.mp4", &blob_ref).await;

        let embedder = MockEmbedder::new();
        let payload = IngestJobPayload { entity_id: Some("ent_clip".into()), ..Default::default() };
        let outcome = process_ingest_job(&conn, &store, &embedder, "ws_1", payload, 100).await.unwrap();

        match outcome {
            IngestOutcome::Unsupported { kind, blocked_by } => {
                assert_eq!(kind, "video");
                assert!(blocked_by.contains("#89"));
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }

        let mut seg_rows = conn.query("SELECT COUNT(*) FROM entities WHERE type='segment'", ()).await.unwrap();
        let count: i64 = seg_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(count, 0);
        assert!(embedder.calls.lock().unwrap().is_empty());

        let mut parent_rows = conn
            .query(
                "SELECT json_extract(attrs,'$.ingest_status'), json_extract(attrs,'$.ingest_blocked_by') FROM entities WHERE id='ent_clip'",
                (),
            )
            .await
            .unwrap();
        let row = parent_rows.next().await.unwrap().unwrap();
        let status: String = row.get(0).unwrap();
        let blocked_by: String = row.get(1).unwrap();
        assert_eq!(status, "unsupported");
        assert!(blocked_by.contains("#89"));

        let mut evt_rows = conn.query("SELECT type FROM events WHERE entity_id='ent_clip'", ()).await.unwrap();
        let evt_type: String = evt_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(evt_type, "ingest.unsupported");
    }

    #[tokio::test]
    async fn missing_entity_id_is_rejected_without_partial_writes() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let conn = fresh_conn("test_ingest_missing_entity.db").await;

        let embedder = MockEmbedder::new();
        let payload = IngestJobPayload::default();
        let result = process_ingest_job(&conn, &store, &embedder, "ws_1", payload, 100).await;

        assert!(matches!(result, Err(IngestError::MissingEntityId)));
    }

    #[tokio::test]
    async fn missing_blob_ref_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let conn = fresh_conn("test_ingest_missing_blob.db").await;

        conn.execute(
            "INSERT INTO entities (id, workspace_id, module, type, attrs, created_at, updated_at) \
             VALUES ('ent_no_blob', 'ws_1', 'files', 'file', '{}', 0, 0)",
            (),
        )
        .await
        .unwrap();

        let embedder = MockEmbedder::new();
        let payload = IngestJobPayload { entity_id: Some("ent_no_blob".into()), ..Default::default() };
        let result = process_ingest_job(&conn, &store, &embedder, "ws_1", payload, 100).await;

        assert!(matches!(result, Err(IngestError::NoContent)));
    }
}
