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
use worker::{Fetch, Method, Request, RequestInit};

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
            FetchError::Http(_) => true,       // network / connection level
            FetchError::Status(code) => {
                *code >= 500
                    || *code == 429             // rate limit, may lift
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
        return Err(FetchError::Ssrf(format!(
            "disallowed scheme: {}",
            parsed.scheme()
        )));
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| FetchError::Ssrf("missing host".into()))?;

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
async fn http_get(url: &str, etag: Option<&str>, last_modified: Option<&str>) -> Result<(u16, String, Option<String>, Option<String>), FetchError> {
    guard_public_url(url)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get);

    let headers = worker::Headers::new();
    if let Some(etag) = etag {
        headers
            .set("If-None-Match", etag)
            .map_err(|e| FetchError::Http(e.to_string()))?;
    }
    if let Some(lm) = last_modified {
        headers
            .set("If-Modified-Since", lm)
            .map_err(|e| FetchError::Http(e.to_string()))?;
    }
    init.with_headers(headers);

    let req = Request::new_with_init(url, &init).map_err(|e| FetchError::Http(e.to_string()))?;

    let mut resp = Fetch::Request(req)
        .send()
        .await
        .map_err(|e| FetchError::Http(e.to_string()))?;

    let status = resp.status_code();

    let etag = resp.headers().get("etag").ok().flatten();
    let last_modified = resp.headers().get("last-modified").ok().flatten();

    let body = resp
        .text()
        .await
        .map_err(|e| FetchError::Http(e.to_string()))?;

    Ok((status, body, etag, last_modified))
}

/// Fetch and parse a single RSS/Atom feed URL.  Callers persist the returned
/// etag/last_modified via `store` for the next cycle.
pub async fn fetch_feed(
    url: &str,
    prior_etag: Option<&str>,
    prior_last_modified: Option<&str>,
) -> Result<FetchOutcome, FetchError> {
    let (status, body, etag, last_modified) = http_get(url, prior_etag, prior_last_modified).await?;

    if status == 304 {
        return Ok(FetchOutcome::NotModified);
    }
    if status >= 400 {
        return Err(FetchError::Status(status));
    }

    let feed = parser::parse(body.as_bytes())?;

    Ok(FetchOutcome::Updated(Box::new(FetchedFeed {
        feed,
        raw_body: body,
        etag,
        last_modified,
    })))
}

/// Fetch the full text of a single article URL using CSS selectors.
/// Only called for feeds with `extraction_level = 'full_text'`.
/// The `article.url` originates from third-party feed data, so
/// `guard_public_url` is applied here too.
pub async fn extract_full_text(url: &str) -> Result<String, FetchError> {
    let (status, body, _etag, _lm) = http_get(url, None, None).await?;

    if status >= 400 {
        return Err(FetchError::Status(status));
    }

    let document = Html::parse_document(&body);

    // Ordered list of content selectors, from most specific to fallback.
    let selectors = [
        "article",
        "main",
        ".post-content",
        ".entry-content",
        "#content",
        ".content",
        ".article-body",
    ];

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
        let text: String = document
            .select(&sel)
            .map(|el| el.text().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n\n");
        let trimmed = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    Err(FetchError::Extraction("no readable content found".into()))
}
