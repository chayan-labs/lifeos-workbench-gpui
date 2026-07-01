# Media intelligence - lifeos-ingest

> Goal: ask "find the clip where I said X" / "find the image showing Y" / "find the doc about Z" across your whole media library.
> **Honest boundary:** memvec is text-only. It cannot see audio, video, or images. Accuracy depends entirely on a media→text front-end we must build. This document specifies it.

---

## 1. The memvec reality check

`memvec.py` = MiniLM-384 (`all-MiniLM-L6-v2`), a **text-only** sentence embedder into sqlite-vec.
Feed it a video and nothing happens.
So "find the clip where I said X" works **if and only if** we first transcribe audio into timestamped text segments, embed those, and store the timestamp.
**The pipeline, not memvec, is the work.** memvec is reused unchanged.

**One honest gap:** this gives *semantic-of-spoken/written-content* search. For **true visual similarity** ("find the frame that looks like this"), MiniLM can't help - add **CLIP** image embeddings as a *second* vector space in the same sqlite-vec DB (different dimension/collection). Ship transcription/caption/parse first (covers ~95% of "find where I said/showed X"); add CLIP only if reverse-image/visual search is genuinely wanted.

---

## 2. The pipeline

```
media file ──► route by MIME ──► extract TEXT ──► segment ──► memvec + FTS5
  audio/video → Whisper (timestamped segments)
  image       → vision-LLM caption / OCR
  pdf/docx    → text extraction (pdfium/poppler)
                         │
                         ▼
        each segment = child entity:
        type=segment, parent=<asset>, attrs={ t_start, t_end, text, page? }
                         │
                         ▼
        memvec embeds segment.text  → entity_vec (in lifeos-derived.db)
        FTS5 indexes segment.text   → entities_fts
```

**Query path:** "where I said X" → memvec matches a `segment` → return `t_start` + parent `asset` → deep-link to that exact timestamp/page.

**Implemented (issue #88):** `services/lifeos-ingest` is the real orchestrator - MIME routing,
job dispatch, segment-entity creation, and triggering memvec indexing, which is #88's actual
scope (not the heavy extractors themselves). `lifeos-drain` claims an `ingest` job and calls
`lifeos_ingest::process_ingest_job` directly as a library (both crates share this workspace,
no subprocess). `route_by_mime` is real end-to-end for **plain text** (`.txt/.md/.markdown/
.csv/.log/.json`): the blob is read via `lifeos_vcs::read_blob`, split into blank-line-
separated paragraphs (`chunk_plain_text` - honest chunking, no fabricated NLP segmentation),
each paragraph becomes a `type=segment` child entity (`attrs={text}`, `parent_id=<asset>`),
and `Embedder::embed` fires per segment (a `SubprocessEmbedder` shelling out to
`memvec.py embed`, or a logging `NoopEmbedder` when `LIFEOS_MEMVEC` is unset - see
docs/MANUAL-SETUP.md §88). The parent entity's `attrs.transcript_ref` rollup (§5 below) is set
to a fresh `lifeos_vcs::store_blob` of the full extracted text.

**Implemented (issue #89):** audio (`.mp3/.wav/.m4a`) transcription is real. `services/
lifeos-ingest/src/audio.rs::decode_to_16k_mono_f32` decodes any symphonia-supported container
to 16kHz mono f32 PCM (pure Rust, no ffmpeg); a `Transcriber` trait (DI, same shape as
`Embedder`) runs it through `whisper-rs` (`WhisperTranscriber`, CPU-bound inference on
`spawn_blocking`) against a local GGML model (`LIFEOS_WHISPER_MODEL`, see
docs/MANUAL-SETUP.md §89). Each returned span becomes a `type=segment` child entity with real
`attrs.t_start`/`attrs.t_end` (seconds) - the locator fields plain text (#88) had no value for -
plus `attrs.text`, embedded the same way plain-text segments are. When `LIFEOS_WHISPER_MODEL`
is unset, `lifeos-drain` uses a `NoopTranscriber` that fails the job loudly rather than
silently producing zero segments - a missing transcriber for a MIME class the router says it
can handle is a real capability gap, not an honest "still stubbed" state.

Video containers (`.mov/.webm/.mp4` - no general video-container demuxer here) and image/PDF/
docx (**#90**, vision-LLM captioning / OCR / pdfium text extraction) remain honest stubs:
`route_by_mime` returns `Unsupported{kind, blocked_by}` naming the exact gap - never a silent
no-op. No segments are created for an unsupported kind; the parent entity's
`attrs.ingest_status="unsupported"` + `attrs.ingest_blocked_by` record why, and an
`ingest.unsupported` event is emitted - the job still completes (nothing to retry until #90
lands for images/PDF or a video-container demuxer exists), it just honestly produces zero
segments rather than fabricating any.

---

## 3. Components (Rust-heavy)

| Step | Tool | Class | Rust |
|---|---|---|---|
| Orchestrator `lifeos-ingest` | dispatch by MIME, manage segments, write entities | BUILD | 🦀 |
| Transcription (audio/video) | **whisper-rs** or **candle-whisper** (fallback whisper.cpp) | FORK | 🦀 |
| Image caption / OCR | vision-LLM (Haiku) for caption; **tesseract** for OCR | reuse/fork | mixed |
| PDF / doc text | **pdfium** / poppler bindings | fork | C |
| Embedding | **memvec.py** (MiniLM-384, sqlite-vec) | reuse as-is | Python |
| (optional) visual search | **candle-clip** image embeddings | fork | 🦀 |

- Heavy transcription runs on the **Mac heavy lane via `jobs`**: the bot enqueues `{kind:'ingest', payload:{blob_ref}}`; `lifeos-drain` claims it; `lifeos-ingest` processes it.
- Vectors land in the **separate un-synced `lifeos-derived.db`** (per [DATA-MODEL.md](./DATA-MODEL.md) §5) - rebuildable, never synced.

---

## 4. Triggers & freshness

- On `file.imported` / `asset.generated` / `version.created` → enqueue an ingest job.
- On a new file **version**, re-derive segments for that version so search reflects the latest content (old versions remain searchable via their snapshot, see [VERSIONING.md](./VERSIONING.md)).
- Reading-module articles and Email bodies also flow through the text path (no transcription needed) → one unified semantic index across *all* content.

---

## 5. Data shape

- `segment` entities are cheap children of the media `asset`; they carry the searchable text + locator (`t_start/t_end` for AV, `page` for docs, `bbox?` for image regions).
- The `asset` entity keeps a rollup (`attrs.transcript_ref` → full transcript blob) for display.
- Deleting/re-versioning an asset cascades a re-index of its segments.

---

## 6. Verification

- Ingest a known video → assert N `segment` entities with monotonic timestamps; query a phrase spoken at 3:12 → top hit's `t_start ≈ 192s`.
- Ingest a PDF → query a phrase on page 7 → hit carries `page=7`.
- Confirm vectors live only in `lifeos-derived.db` and survive a rebuild (`rm lifeos-derived.db && reindex`).
- (If CLIP added) upload an image, query by a similar image → visual match in the separate CLIP collection.
