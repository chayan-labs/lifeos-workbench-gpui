//! PDF and docx text extraction (issue #90, docs/MEDIA-INTELLIGENCE.md §3).
//!
//! Deliberate deviation from `docs/RUST-COMPONENTS.md`'s "pdfium/poppler"
//! wording: `pdf-extract` is pure Rust (no C bindgen/system lib), and docx
//! is just a zip of XML - pulling `<w:t>` text runs directly avoids a full
//! docx-rs dependency. Both match "reuse before build" over the heaviest
//! available option.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::io::Read;

/// Extracts text per page. Inference is CPU-bound sync work, so callers
/// should run this on `spawn_blocking`, same pattern as `run_whisper`.
pub fn extract_pdf_pages(bytes: &[u8]) -> Result<Vec<String>, String> {
    pdf_extract::extract_text_from_mem_by_pages(bytes).map_err(|e| format!("pdf text extraction failed: {e}"))
}

/// Reads `word/document.xml` out of a `.docx` (a zip archive) and pulls the
/// plain text out of every `<w:t>` run, in document order. No paragraph/run
/// formatting is preserved - just the searchable text, same honesty policy
/// as `chunk_plain_text`'s "no fabricated NLP segmentation" comment.
pub fn extract_docx_text(bytes: &[u8]) -> Result<String, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("docx is not a valid zip: {e}"))?;
    let mut document_xml = archive
        .by_name("word/document.xml")
        .map_err(|e| format!("docx missing word/document.xml: {e}"))?;
    let mut xml = String::new();
    document_xml
        .read_to_string(&mut xml)
        .map_err(|e| format!("failed to read word/document.xml: {e}"))?;
    drop(document_xml);

    extract_text_runs(&xml)
}

fn extract_text_runs(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut text = String::new();
    let mut in_text_run = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"t" => in_text_run = true,
            Ok(Event::End(e)) if e.local_name().as_ref() == b"t" => in_text_run = false,
            Ok(Event::End(e)) if e.local_name().as_ref() == b"p" => text.push_str("\n\n"),
            Ok(Event::Text(e)) if in_text_run => {
                let decoded = e.unescape().map_err(|err| format!("docx xml decode failed: {err}"))?;
                text.push_str(&decoded);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("docx xml parse failed: {e}")),
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
}

/// Test-only docx byte builder, shared with `lib.rs`'s ingest-pipeline tests
/// so both layers exercise a real (if minimal) docx rather than a fixture
/// file, matching this crate's synthetic-data test convention.
#[cfg(test)]
pub(crate) mod tests_support {
    use std::io::Write;

    pub(crate) fn synth_docx_bytes(paragraphs: &[&str]) -> Vec<u8> {
        let body: String =
            paragraphs.iter().map(|p| format!("<w:p><w:r><w:t>{p}</w:t></w:r></w:p>")).collect();
        let document_xml = format!(
            "<?xml version=\"1.0\"?><w:document xmlns:w=\"ns\"><w:body>{body}</w:body></w:document>"
        );

        let mut buf = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut buf);
            let mut writer = zip::ZipWriter::new(cursor);
            let options: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            writer.start_file("word/document.xml", options).unwrap();
            writer.write_all(document_xml.as_bytes()).unwrap();
            writer.finish().unwrap();
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::synth_docx_bytes;
    use super::*;

    #[test]
    fn extracts_paragraph_text_from_docx() {
        let bytes = synth_docx_bytes(&["Hello world", "Second paragraph"]);
        let text = extract_docx_text(&bytes).unwrap();
        assert!(text.contains("Hello world"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn rejects_non_zip_bytes() {
        let result = extract_docx_text(b"not a zip file");
        assert!(result.is_err());
    }
}
