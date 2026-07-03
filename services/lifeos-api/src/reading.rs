//! Article fetching for the Reading module (issue #61, docs/MODULES.md
//! §3.6). Unlike Kite/WhatsApp/browser-use, fetching a public URL needs no
//! owned credentials - so this client is always wired in production
//! (`reading_from_config` ignores `config` and always returns `Some`);
//! tests inject `reading::mock::MockArticleFetcher` instead of hitting the
//! network, following the same trait+mock pattern as every other external
//! client in this crate (`kite.rs`, `whatsapp.rs`, `browser.rs`).
//!
//! Full Mozilla Readability.js extraction (the vendored `external/readability`
//! submodule, run via a Node/jsdom subprocess) is deferred - `parse_article`
//! in `routes/reading.rs` does a lighter, dependency-light HTML→text
//! extraction with the `scraper` crate today. See docs/MODULES.md §3.6 for
//! the explicit scope note.

use crate::error::{ApiError, ApiResult};
use async_trait::async_trait;

#[async_trait]
pub trait ArticleFetcher: Send + Sync {
    /// Fetches `url` and returns the raw response body (HTML).
    async fn fetch(&self, url: &str) -> ApiResult<String>;
}

pub struct HttpArticleFetcher {
    http: reqwest::Client,
}

impl HttpArticleFetcher {
    pub fn new() -> Self {
        Self { http: reqwest::Client::new() }
    }
}

impl Default for HttpArticleFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ArticleFetcher for HttpArticleFetcher {
    async fn fetch(&self, url: &str) -> ApiResult<String> {
        let resp = self.http.get(url).send().await.map_err(|e| {
            tracing::error!("article fetch failed for {url}: {e}");
            ApiError::Upstream("article fetch failed".into())
        })?;
        if !resp.status().is_success() {
            return Err(ApiError::Upstream(format!("article fetch returned {}", resp.status())));
        }
        resp.text().await.map_err(|e| {
            tracing::error!("article body decode failed for {url}: {e}");
            ApiError::Upstream("malformed article response".into())
        })
    }
}

/// In-memory fake for tests - no real network needed to exercise the API
/// surface.
pub mod mock {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct MockArticleFetcher {
        pages: Mutex<HashMap<String, String>>,
        /// Every URL fetched, in order.
        pub calls: Mutex<Vec<String>>,
    }

    impl MockArticleFetcher {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn seed(&self, url: &str, html: &str) {
            self.pages.lock().unwrap().insert(url.to_string(), html.to_string());
        }
    }

    #[async_trait]
    impl ArticleFetcher for MockArticleFetcher {
        async fn fetch(&self, url: &str) -> ApiResult<String> {
            self.calls.lock().unwrap().push(url.to_string());
            self.pages.lock().unwrap().get(url).cloned().ok_or_else(|| ApiError::NotFound(format!("no mock page seeded for {url}")))
        }
    }
}
