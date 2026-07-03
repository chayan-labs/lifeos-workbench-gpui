//! Tesseract OCR for images (issue #90, docs/MEDIA-INTELLIGENCE.md §3).
//!
//! OCR is a supplement to captioning, not the sole extractor for an image -
//! unlike `Transcriber`/`Captioner`, a missing/failing OCR degrades to empty
//! text rather than failing the whole job, same policy as `NoopEmbedder`.
//! `TesseractOcr` shells out to the `tesseract` CLI (no bindgen), same
//! subprocess DI shape as `SubprocessEmbedder`.

use async_trait::async_trait;
use std::io::Write;

/// Extracts any text visible in an image (signage, screenshots, scanned
/// documents saved as images, etc).
#[async_trait]
pub trait Ocr: Send + Sync {
    async fn extract_text(&self, image_bytes: &[u8]) -> Result<String, String>;
}

/// Used when `LIFEOS_TESSERACT_BIN` is unset. Unlike `NoopCaptioner`, this
/// degrades gracefully: captioning alone still makes the image searchable.
pub struct NoopOcr;

#[async_trait]
impl Ocr for NoopOcr {
    async fn extract_text(&self, _image_bytes: &[u8]) -> Result<String, String> {
        Ok(String::new())
    }
}

/// Shells out to the `tesseract` CLI against a temp file, same reasoning as
/// `SubprocessEmbedder`: no C bindgen, just a system binary (installed via
/// Nix, see docs/MANUAL-SETUP.md §90).
pub struct TesseractOcr {
    pub bin_path: String,
}

#[async_trait]
impl Ocr for TesseractOcr {
    async fn extract_text(&self, image_bytes: &[u8]) -> Result<String, String> {
        let bin_path = self.bin_path.clone();
        let image_bytes = image_bytes.to_vec();
        tokio::task::spawn_blocking(move || run_tesseract(&bin_path, &image_bytes))
            .await
            .map_err(|e| format!("ocr task panicked: {e}"))?
    }
}

fn run_tesseract(bin_path: &str, image_bytes: &[u8]) -> Result<String, String> {
    let mut input = tempfile::Builder::new()
        .suffix(".png")
        .tempfile()
        .map_err(|e| format!("failed to create ocr temp input: {e}"))?;
    input.write_all(image_bytes).map_err(|e| format!("failed to write ocr temp input: {e}"))?;
    input.flush().map_err(|e| format!("failed to flush ocr temp input: {e}"))?;

    let output_stem = tempfile::Builder::new()
        .tempfile()
        .map_err(|e| format!("failed to create ocr temp output: {e}"))?
        .path()
        .to_path_buf();

    let status = std::process::Command::new(bin_path)
        .arg(input.path())
        .arg(&output_stem)
        .status()
        .map_err(|e| format!("failed to spawn tesseract: {e}"))?;
    if !status.success() {
        return Err(format!("tesseract exited with status {status}"));
    }

    let text_path = output_stem.with_extension("txt");
    let text = std::fs::read_to_string(&text_path).map_err(|e| format!("failed to read tesseract output: {e}"))?;
    let _ = std::fs::remove_file(&text_path);
    Ok(text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_ocr_degrades_to_empty_text() {
        let result = NoopOcr.extract_text(b"fake-bytes").await;
        assert_eq!(result, Ok(String::new()));
    }
}
