//! Vision-LLM captioning for images (issue #90, docs/MEDIA-INTELLIGENCE.md §3).
//!
//! Same DI-trait shape as `Transcriber`/`Embedder`: a `NoopCaptioner` fails
//! loudly (routing claims image support, so a missing captioner is a real
//! capability gap, not a degrade-safely case) and `HaikuCaptioner` calls the
//! Anthropic Messages API directly over `reqwest` - no SDK dependency.

use async_trait::async_trait;
use base64::Engine;
use serde_json::json;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const CAPTION_MODEL: &str = "claude-haiku-4-5-20251001";
const CAPTION_PROMPT: &str =
    "Describe this image in one or two concise sentences, focused on what it shows so it can be found later by a text search.";

/// Produces a short searchable caption for one image.
#[async_trait]
pub trait Captioner: Send + Sync {
    async fn caption(&self, image_bytes: &[u8], mime: &str) -> Result<String, String>;
}

/// Used when `ANTHROPIC_API_KEY` is unset. Fails loudly, same reasoning as
/// `NoopTranscriber`: `route_by_mime` promises image support, so silently
/// producing zero segments would hide a real capability gap.
pub struct NoopCaptioner;

#[async_trait]
impl Captioner for NoopCaptioner {
    async fn caption(&self, _image_bytes: &[u8], _mime: &str) -> Result<String, String> {
        Err("no vision captioner configured (ANTHROPIC_API_KEY unset)".to_string())
    }
}

/// Real captioning via the Anthropic Messages API (Haiku vision).
pub struct HaikuCaptioner {
    pub api_key: String,
}

#[async_trait]
impl Captioner for HaikuCaptioner {
    async fn caption(&self, image_bytes: &[u8], mime: &str) -> Result<String, String> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(image_bytes);
        let body = json!({
            "model": CAPTION_MODEL,
            "max_tokens": 256,
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "image", "source": { "type": "base64", "media_type": mime, "data": encoded } },
                    { "type": "text", "text": CAPTION_PROMPT }
                ]
            }]
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("anthropic request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("anthropic api error {status}: {text}"));
        }

        let parsed: serde_json::Value =
            resp.json().await.map_err(|e| format!("anthropic response parse failed: {e}"))?;
        parsed
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "anthropic response had no caption text".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_captioner_fails_loudly() {
        let result = NoopCaptioner.caption(b"fake-bytes", "image/png").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ANTHROPIC_API_KEY"));
    }
}
