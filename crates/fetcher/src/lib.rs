//! Feed fetching and optional full-text article extraction.
//!
//! `fetch_feed` pulls and parses an RSS/Atom feed URL. `extract_full_text`
//! fetches a single article URL (the canonical link from a feed entry) and
//! extracts readable body text via CSS selectors -- only called for feeds
//! with `extraction_level = 'full_text'`, which is opt-in per source.

mod ssrf;
mod fetch;
mod extract;

pub use ssrf::*;
pub use fetch::*;
pub use extract::*;

use feed_rs::model::Feed;

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("http request failed: {0}")]
    Http(String),
    #[error("non-success status: {0}")]
    Status(u16),
    #[error("feed parse failed: {0}")]
    Parse(#[from] feed_rs::parser::ParseFeedError),
    #[error("blocked by SSRF guard: {0}")]
    Ssrf(String),
    #[error("full-text extraction failed: {0}")]
    Extraction(String),
}

impl FetchError {
    /// Returns true for errors where retrying makes sense (network blips,
    /// rate limiting, server errors).  Returns false for permanent errors
    /// (4xx client errors, SSRF blocks, parse failures) where retrying
    /// would waste the queue's retry quota.
    pub fn is_transient(&self) -> bool {
        match self {
            FetchError::Http(_) => true, // network / connection level
            FetchError::Status(code) => {
                *code >= 500 || *code == 429 // rate limit, may lift
            }
            FetchError::Parse(_) => false,      // bad XML won't get better
            FetchError::Ssrf(_) => false,       // policy block, won't change
            FetchError::Extraction(_) => false, // parse fail, won't change
        }
    }
}

pub struct FetchedFeed {
    pub feed: Feed,
    pub raw_body: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// Outcome of a conditional fetch: either the feed changed and we parsed
/// it, or the server confirmed nothing changed since last time (304) and
/// there is nothing to re-parse or re-run through the AI pipeline for.
pub enum FetchOutcome {
    Updated(Box<FetchedFeed>),
    NotModified,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- is_transient ----

    #[test]
    fn transient_http_error() {
        assert!(FetchError::Http("connection reset".into()).is_transient());
    }

    #[test]
    fn transient_5xx_status() {
        assert!(FetchError::Status(500).is_transient());
        assert!(FetchError::Status(502).is_transient());
        assert!(FetchError::Status(503).is_transient());
    }

    #[test]
    fn transient_429_status() {
        assert!(FetchError::Status(429).is_transient());
    }

    #[test]
    fn permanent_4xx_status() {
        assert!(!FetchError::Status(400).is_transient());
        assert!(!FetchError::Status(401).is_transient());
        assert!(!FetchError::Status(403).is_transient());
        assert!(!FetchError::Status(404).is_transient());
        assert!(!FetchError::Status(410).is_transient());
    }

    #[test]
    fn permanent_parse_error() {
        let result = feed_rs::parser::parse("not xml".as_bytes());
        let err = result.expect_err("should fail to parse");
        let fetch_err = FetchError::Parse(err);
        assert!(!fetch_err.is_transient());
    }

    #[test]
    fn permanent_ssrf_error() {
        assert!(!FetchError::Ssrf("blocked by policy".into()).is_transient());
    }

    #[test]
    fn permanent_extraction_error() {
        assert!(!FetchError::Extraction("no content".into()).is_transient());
    }
}
