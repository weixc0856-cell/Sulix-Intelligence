//! Unified Signal Health — canonical 4-factor composite score.
//!
//! This is the single source of truth for signal health computation.
//! Both Radar and Detail use this formula.
//!
//! Factors (weights):
//!   activity  0.35 — articles per day (capped)
//!   diversity 0.25 — unique sources (capped)
//!   quality   0.20 — avg score / 10
//!   velocity  0.20 — trend-based momentum

/// Raw metrics needed to compute signal health.
pub struct SignalHealthMetrics {
    pub article_count: i64,
    pub source_count: i64,
    pub avg_score: f64,
    pub trend: String,
    pub days_active: f64,
}

/// Compute the canonical signal health score (0.0–1.0).
pub fn calculate_health(metrics: &SignalHealthMetrics) -> f64 {
    let activity = (metrics.article_count as f64 / metrics.days_active).min(10.0) / 10.0;
    let diversity = (metrics.source_count as f64).min(15.0) / 15.0;
    let quality = (metrics.avg_score / 10.0).clamp(0.0, 1.0);
    let velocity: f64 = match metrics.trend.as_str() {
        "rising" => 1.0,
        "stable" => 0.5,
        _ => 0.15,
    };

    let score = 0.35 * activity + 0.25 * diversity + 0.20 * quality + 0.20 * velocity;
    (score * 100.0).round() / 100.0
}

/// Compute health breakdown components (for Radar display).
pub fn health_breakdown(metrics: &SignalHealthMetrics) -> (f64, f64, f64, f64) {
    let activity = (metrics.article_count as f64 / metrics.days_active).min(10.0) / 10.0;
    let diversity = (metrics.source_count as f64).min(15.0) / 15.0;
    let quality = (metrics.avg_score / 10.0).clamp(0.0, 1.0);
    let velocity: f64 = match metrics.trend.as_str() {
        "rising" => 1.0,
        "stable" => 0.5,
        _ => 0.15,
    };
    (
        (activity * 100.0).round() / 100.0,
        (diversity * 100.0).round() / 100.0,
        (quality * 100.0).round() / 100.0,
        (velocity * 100.0).round() / 100.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_activity_rising() {
        let m = SignalHealthMetrics {
            article_count: 50,
            source_count: 12,
            avg_score: 8.5,
            trend: "rising".into(),
            days_active: 7.0,
        };
        let s = calculate_health(&m);
        assert!(s > 0.7, "high activity rising should score >0.7, got {s}");
    }

    #[test]
    fn low_activity_decaying() {
        let m = SignalHealthMetrics {
            article_count: 2,
            source_count: 1,
            avg_score: 3.0,
            trend: "declining".into(),
            days_active: 7.0,
        };
        let s = calculate_health(&m);
        assert!(s < 0.5, "low activity decaying should score <0.5, got {s}");
    }

    #[test]
    fn stable_mid_range() {
        let m = SignalHealthMetrics {
            article_count: 15,
            source_count: 6,
            avg_score: 6.0,
            trend: "stable".into(),
            days_active: 7.0,
        };
        let s = calculate_health(&m);
        assert!(s > 0.3 && s < 0.8, "stable should score 0.3-0.8, got {s}");
    }
}
