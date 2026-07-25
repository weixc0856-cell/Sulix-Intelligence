//! Signal Intelligence route handlers.
//!
//! - `GET /api/intelligence/radar` — Intelligence Radar dashboard

use serde_json::json;
use worker::*;

use store::Store;
use store::domain::signal::projection::build_radar_response;
use store::domain::signal::health::calculate_signal_health;

use crate::shared::response;

/// GET /api/intelligence/radar — Intelligence Radar dashboard.
///
/// Returns active signal threads with health scores, ranked for the
/// radar view. Replaces the old score-bucket signal dashboard.
pub async fn radar(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;

    let threads = match store.get_active_signal_threads(50).await {
        Ok(t) => t,
        Err(e) => {
            console_log!("[Sulix:radar] get_active_signal_threads failed: {e}");
            return response::json_err_internal("radar query failed");
        }
    };

    // Build health scores for each thread using instance data
    let mut health_scores = Vec::with_capacity(threads.len());
    for input in &threads {
        let days_active = if input.instances.len() > 1 {
            let first = input.instances.last().map(|i| i.generated_at).unwrap_or(now);
            let last = input.instances.first().map(|i| i.generated_at).unwrap_or(now);
            ((last - first) as f64 / 86400.0).max(1.0)
        } else {
            1.0
        };

        let health = calculate_signal_health(
            input.recent_article_count,
            input.source_count,
            input.instances.first().map(|i| i.score).unwrap_or(0.0),
            &input.trend,
            days_active,
        );
        health_scores.push(health);
    }

    // Project onto radar DTO and sort by radar_score
    let mut radar = build_radar_response(threads, health_scores, now);
    radar.signals.sort_by(|a, b| {
        let ha = a.health.score;
        let hb = b.health.score;
        hb.partial_cmp(&ha).unwrap_or(std::cmp::Ordering::Equal)
    });

    response::json_ok(json!(radar))
}
