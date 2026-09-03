//! Minimal HTTP response helpers for routes registered in worker-entry.
//!
//! Replicates the pub(crate) helpers in `api/src/shared/response.rs` so the
//! routes moved out of `api` (context / agent) stay self-contained. This is
//! deliberately small — only what the moved handlers use.

use serde_json::Value;
use worker::*;

pub(crate) fn cors_headers(resp: &mut Response) {
    let h = resp.headers_mut();
    let _ = h.set("Access-Control-Allow-Origin", "*");
    let _ = h.set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS");
    let _ = h.set("Access-Control-Allow-Headers", "Content-Type");
    let _ = h.set("X-Content-Type-Options", "nosniff");
    let _ = h.set("Cache-Control", "public, max-age=60");
}

pub(crate) fn json_ok(v: Value) -> Result<Response> {
    let mut resp = Response::from_json(&v)?;
    cors_headers(&mut resp);
    Ok(resp)
}

pub(crate) fn json_err(status: u16, msg: &str) -> Result<Response> {
    let mut resp = Response::error(msg, status)?;
    cors_headers(&mut resp);
    Ok(resp)
}

/// Log an internal error server-side and return a generic 500 response.
/// Never passes internal details to the HTTP client.
pub(crate) fn json_err_internal(msg: &str) -> Result<Response> {
    console_log!("[Sulix:internal] error: {msg}");
    json_err(500, "Internal server error")
}
