//! Pulls a feed URL through the Workers runtime's `fetch()` binding and
//! parses it with `feed-rs`.
//!
//! Deliberately does NOT use `reqwest`: the Workers wasm32-unknown-unknown
//! target has no TCP sockets, so `reqwest` either fails to build or fails
//! at runtime. `worker::Fetch` shells out to the JS `fetch()` global that
//! the Workers runtime actually provides.

use feed_rs::model::Feed;
use feed_rs::parser;
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
    Updated(FetchedFeed),
    NotModified,
}

/// Basic SSRF guard, in the spirit of NewsBlur's `validate_public_url`:
/// reject IP-literal hosts pointing at loopback/private/link-local ranges
/// (including the 169.254.169.254 cloud metadata address) and obvious
/// localhost aliases before we ever hand the URL to `fetch()`.
///
/// Caveat, stated plainly: this only catches IP-literal and localhost-alias
/// URLs. It does NOT prevent DNS rebinding (a hostname that resolves to a
/// private IP at fetch time) -- Workers' `fetch()` doesn't expose a way to
/// resolve DNS yourself and check the IP before the request goes out, so
/// full protection against rebinding isn't achievable purely at this layer
/// on the edge runtime. This guard only matters once feed URLs can come
/// from something other than a list you curate yourself; while the source
/// list stays self-maintained, the actual exposure is low.
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
                    || v4 == std::net::Ipv4Addr::new(169, 254, 169, 254) // cloud metadata
            }
            IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00, // ULA
        };
        if blocked {
            return Err(FetchError::Ssrf(format!("IP-literal host in blocked range: {ip}")));
        }
    }

    Ok(())
}

/// Fetch and parse a single RSS/Atom feed URL. Pass in the ETag and/or
/// Last-Modified value stored from the previous successful fetch (if any)
/// so the server can reply 304 Not Modified when nothing changed -- saves
/// bandwidth and, more importantly, skips re-running the AI pipeline on
/// content that hasn't actually changed. Callers persist the returned
/// etag/last_modified via `store` for the next cycle.
pub async fn fetch_feed(
    url: &str,
    prior_etag: Option<&str>,
    prior_last_modified: Option<&str>,
) -> Result<FetchOutcome, FetchError> {
    guard_public_url(url)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get);

    let mut headers = worker::Headers::new();
    if let Some(etag) = prior_etag {
        headers
            .set("If-None-Match", etag)
            .map_err(|e| FetchError::Http(e.to_string()))?;
    }
    if let Some(lm) = prior_last_modified {
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

    if resp.status_code() == 304 {
        return Ok(FetchOutcome::NotModified);
    }

    if resp.status_code() >= 400 {
        return Err(FetchError::Status(resp.status_code()));
    }

    let resp_headers = resp.headers();
    let etag = resp_headers.get("etag").ok().flatten();
    let last_modified = resp_headers.get("last-modified").ok().flatten();

    let body = resp
        .text()
        .await
        .map_err(|e| FetchError::Http(e.to_string()))?;

    let feed = parser::parse(body.as_bytes())?;

    Ok(FetchOutcome::Updated(FetchedFeed {
        feed,
        raw_body: body,
        etag,
        last_modified,
    }))
}
