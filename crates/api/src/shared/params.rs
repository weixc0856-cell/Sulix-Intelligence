use worker::wasm_bindgen::JsValue;
use worker::{RouteContext, Url};

pub(crate) fn parse_limit(url: &Url) -> u32 {
    url.query_pairs().find(|(k, _)| k == "limit").and_then(|(_, v)| v.parse().ok()).unwrap_or(30)
}

pub(crate) fn parse_offset(url: &Url) -> u32 {
    url.query_pairs().find(|(k, _)| k == "offset").and_then(|(_, v)| v.parse().ok()).unwrap_or(0)
}

pub(crate) fn param_i64<D>(ctx: &RouteContext<D>, name: &str) -> Option<i64> {
    ctx.param(name)?.parse().ok()
}

/// Format a unix timestamp (seconds) as YYYY-MM-DD using js_sys::Date.
pub(crate) fn fmt_date_ymd(ts_secs: i64) -> String {
    let d = js_sys::Date::new(&JsValue::from_f64((ts_secs as f64) * 1000.0));
    format!("{:04}-{:02}-{:02}", d.get_full_year(), d.get_month() + 1, d.get_date())
}

/// Format a unix timestamp (seconds) as ISO 8601 UTC.
pub(crate) fn fmt_datetime_iso(ts_secs: i64) -> String {
    let d = js_sys::Date::new(&JsValue::from_f64((ts_secs as f64) * 1000.0));
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        d.get_full_year(),
        d.get_month() + 1,
        d.get_date(),
        d.get_hours(),
        d.get_minutes(),
        d.get_seconds(),
    )
}
