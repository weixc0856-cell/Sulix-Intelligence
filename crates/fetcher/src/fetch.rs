use crate::ssrf::guard_public_url;
use crate::{FetchError, FetchOutcome, FetchedFeed};
use feed_rs::parser;
use worker::{AbortSignal, Fetch, Method, Request, RequestInit};

/// Low-level HTTP GET used by both `fetch_feed` and `extract_full_text`.
/// Returns the full response so callers can choose between text/json/status.
/// `timeout_ms` is applied via `AbortSignal::timeout` (noop on wasm32? tested).
pub(crate) async fn http_get(
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

    let ws_signal = worker::web_sys::AbortSignal::timeout_with_u32(timeout_ms);
    let signal = AbortSignal::from(ws_signal);
    let mut resp = Fetch::Request(req).send_with_signal(&signal).await.map_err(|e| FetchError::Http(e.to_string()))?;

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
