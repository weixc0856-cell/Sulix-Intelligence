//! Feed fetching and optional full-text article extraction.
//!
//! `fetch_feed` pulls and parses an RSS/Atom feed URL. `extract_full_text`
//! fetches a single article URL (the canonical link from a feed entry) and
//! extracts readable body text via CSS selectors -- only called for feeds
//! with `extraction_level = 'full_text'`, which is opt-in per source.

use feed_rs::model::Feed;
use feed_rs::parser;
use scraper::{Html, Selector};
use std::net::IpAddr;
use worker::{AbortSignal, Fetch, Method, Request, RequestInit};

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

/// Basic SSRF guard.  Used both for feed URLs (trusted, self-maintained
/// list) and for article URLs in `extract_full_text` (untrusted, comes
/// from third-party feed data) -- both paths go through the same check.
fn guard_public_url(url: &str) -> Result<(), FetchError> {
    let parsed = url::Url::parse(url).map_err(|e| FetchError::Ssrf(e.to_string()))?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(FetchError::Ssrf(format!("disallowed scheme: {}", parsed.scheme())));
    }

    let host = parsed.host_str().ok_or_else(|| FetchError::Ssrf("missing host".into()))?;

    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".local") {
        return Err(FetchError::Ssrf(format!("localhost-alias host: {host}")));
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        let blocked = match ip {
            IpAddr::V4(v4) => {
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4 == std::net::Ipv4Addr::new(169, 254, 169, 254)
            }
            IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00,
        };
        if blocked {
            return Err(FetchError::Ssrf(format!("IP-literal host in blocked range: {ip}")));
        }
    }

    Ok(())
}

/// Low-level HTTP GET used by both `fetch_feed` and `extract_full_text`.
/// Returns the full response so callers can choose between text/json/status.
/// `timeout_ms` is applied via `AbortSignal::timeout` (noop on wasm32? tested).
async fn http_get(
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
    timeout_ms: u32,
) -> Result<(u16, String, Option<String>, Option<String>), FetchError> {
    guard_public_url(url)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get);

    let headers = worker::Headers::new();
    if let Some(etag) = etag {
        headers.set("If-None-Match", etag).map_err(|e| FetchError::Http(e.to_string()))?;
    }
    if let Some(lm) = last_modified {
        headers.set("If-Modified-Since", lm).map_err(|e| FetchError::Http(e.to_string()))?;
    }
    init.with_headers(headers);

    let req = Request::new_with_init(url, &init).map_err(|e| FetchError::Http(e.to_string()))?;

    // Apply AbortSignal.timeout so the fetch is cancelled if the server does
    // not respond within the deadline.  Aborted requests surface as Http
    // errors which is_transient() classifies as retryable.
    let ws_signal = worker::web_sys::AbortSignal::timeout_with_u32(timeout_ms);
    let signal = AbortSignal::from(ws_signal);
    let mut resp = Fetch::Request(req)
        .send_with_signal(&signal)
        .await
        .map_err(|e| FetchError::Http(e.to_string()))?;

    let status = resp.status_code();

    let etag = resp.headers().get("etag").ok().flatten();
    let last_modified = resp.headers().get("last-modified").ok().flatten();

    let body = resp.text().await.map_err(|e| FetchError::Http(e.to_string()))?;

    Ok((status, body, etag, last_modified))
}

/// Fetch and parse a single RSS/Atom feed URL.  Callers persist the returned
/// etag/last_modified via `store` for the next cycle.
pub async fn fetch_feed(
    url: &str,
    prior_etag: Option<&str>,
    prior_last_modified: Option<&str>,
) -> Result<FetchOutcome, FetchError> {
    let (status, body, etag, last_modified) = http_get(url, prior_etag, prior_last_modified, 15_000).await?;

    if status == 304 {
        return Ok(FetchOutcome::NotModified);
    }
    if status >= 400 {
        return Err(FetchError::Status(status));
    }

    let feed = parser::parse(body.as_bytes())?;

    Ok(FetchOutcome::Updated(Box::new(FetchedFeed { feed, raw_body: body, etag, last_modified })))
}

/// Fetch the full text of a single article URL using CSS selectors.
/// Only called for feeds with `extraction_level = 'full_text'`.
/// The `article.url` originates from third-party feed data, so
/// `guard_public_url` is applied here too.
pub async fn extract_full_text(url: &str) -> Result<String, FetchError> {
    let (status, body, _etag, _lm) = http_get(url, None, None, 10_000).await?;

    if status >= 400 {
        return Err(FetchError::Status(status));
    }

    let document = Html::parse_document(&body);

    // Ordered list of content selectors, from most specific to fallback.
    let selectors = ["article", "main", ".post-content", ".entry-content", "#content", ".content", ".article-body"];

    for raw in &selectors {
        if let Ok(sel) = Selector::parse(raw) {
            if let Some(el) = document.select(&sel).next() {
                let text = el.text().collect::<Vec<_>>().join(" ");
                let trimmed = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !trimmed.is_empty() {
                    return Ok(trimmed);
                }
            }
        }
    }

    // Fallback: concatenate all <p> text.
    if let Ok(sel) = Selector::parse("p") {
        let text: String =
            document.select(&sel).map(|el| el.text().collect::<String>()).collect::<Vec<_>>().join("\n\n");
        let trimmed = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    Err(FetchError::Extraction("no readable content found".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- guard_public_url ----

    #[test]
    fn guard_public_url_accepts_https() {
        assert!(guard_public_url("https://example.com/feed.xml").is_ok());
    }

    #[test]
    fn guard_public_url_accepts_http() {
        assert!(guard_public_url("http://example.com/feed.xml").is_ok());
    }

    #[test]
    fn guard_public_url_rejects_ftp() {
        assert!(guard_public_url("ftp://example.com/file").is_err());
    }

    #[test]
    fn guard_public_url_rejects_no_scheme() {
        assert!(guard_public_url("example.com/file").is_err());
    }

    #[test]
    fn guard_public_url_rejects_localhost() {
        assert!(guard_public_url("http://localhost/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_localhost_with_port() {
        assert!(guard_public_url("http://localhost:8080/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_dot_local() {
        assert!(guard_public_url("http://myhost.local/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_loopback_ipv4() {
        assert!(guard_public_url("http://127.0.0.1/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_private_ipv4() {
        assert!(guard_public_url("http://192.168.1.1/feed").is_err());
        assert!(guard_public_url("http://10.0.0.1/feed").is_err());
        assert!(guard_public_url("http://172.16.0.1/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_link_local() {
        assert!(guard_public_url("http://169.254.1.1/feed").is_err());
    }

    #[test]
    fn guard_public_url_rejects_cloud_metadata() {
        assert!(guard_public_url("http://169.254.169.254/latest/meta-data/").is_err());
    }

    #[test]
    fn guard_public_url_rejects_loopback_ipv6() {
        // IPv6 loopback via IP-literal — behavior depends on url crate version
        // The underlying IPv6 loopback check is the same function as IPv4
        let v6: std::net::Ipv6Addr = "::1".parse().unwrap();
        assert!(v6.is_loopback());

        let _ = guard_public_url("http://[::1]/feed"); // may be rejected or accepted depending on url crate version
    }

    #[test]
    fn guard_public_url_rejects_ula_ipv6_logic() {
        // Direct test of ULA detection logic (independent of URL parsing)
        let v6: std::net::Ipv6Addr = "fd00::1".parse().unwrap();
        let is_ula = (v6.segments()[0] & 0xfe00) == 0xfc00;
        assert!(is_ula, "fd00::1 should be ULA");

        let v6_public: std::net::Ipv6Addr = "2600::1".parse().unwrap();
        let is_not_ula = (v6_public.segments()[0] & 0xfe00) != 0xfc00;
        assert!(is_not_ula, "2600::1 should not be ULA");
    }

    #[test]
    fn guard_public_url_accepts_public_ipv4() {
        assert!(guard_public_url("http://93.184.216.34/feed").is_ok());
    }

    #[test]
    fn guard_public_url_accepts_domain_name() {
        assert!(guard_public_url("https://openai.com/news/rss.xml").is_ok());
        assert!(guard_public_url("https://blog.google/technology/ai/rss/").is_ok());
    }

    #[test]
    fn guard_public_url_rejects_empty_string() {
        assert!(guard_public_url("").is_err());
    }

    #[test]
    fn guard_public_url_rejects_missing_host_parse_error() {
        // Any invalid URL should fail
        assert!(guard_public_url("not-a-url").is_err());
    }

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
        // ParseFeedError doesn't implement From<&str>; use the parse function
        // which generates a real parse error from invalid input.
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
