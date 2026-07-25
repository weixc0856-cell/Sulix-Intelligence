//! Signal Health — pure function for multi-dimensional health assessment.
//!
//! Health is a composite of four factors, each normalized 0.0-1.0:
//!   activity  — evidence_velocity (articles per day, capped)
//!   diversity — source breadth (unique sources, capped)
//!   quality   — avg_score / 10.0
//!   velocity  — recent momentum (trend-based)
//!
//! Overall score = weighted average of the four components.
//! This is a pure function — no DB access, testable with unit tests.

use crate::SignalHealth;
use crate::SignalHealthBreakdown;

const WEIGHT_ACTIVITY: f64 = 0.35;
const WEIGHT_DIVERSITY: f64 = 0.25;
const WEIGHT_QUALITY: f64 = 0.20;
const WEIGHT_VELOCITY: f64 = 0.20;

/// Compute signal health from raw metrics.
pub fn calculate_signal_health(
    article_count: i64,
    source_count: i64,
    avg_score: f64,
    trend: &str,
    days_active: f64,
) -> SignalHealth {
    let activity = (article_count as f64 / days_active).min(10.0) / 10.0;
    let diversity = (source_count as f64).min(15.0) / 15.0;
    let quality = (avg_score / 10.0).clamp(0.0, 1.0);
    let velocity = match trend {
        "rising" => 1.0,
        "stable" => 0.5,
        _ => 0.15,
    };

    let score = WEIGHT_ACTIVITY * activity
        + WEIGHT_DIVERSITY * diversity
        + WEIGHT_QUALITY * quality
        + WEIGHT_VELOCITY * velocity;

    SignalHealth {
        score: (score * 100.0).round() / 100.0,
        breakdown: SignalHealthBreakdown {
            activity: (activity * 100.0).round() / 100.0,
            diversity: (diversity * 100.0).round() / 100.0,
            quality: (quality * 100.0).round() / 100.0,
            velocity: (velocity * 100.0).round() / 100.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_activity_rising() {
        let h = calculate_signal_health(50, 12, 8.5, "rising", 7.0);
        assert!(h.score > 0.7);
        assert!(h.breakdown.activity > 0.7);
        assert!(h.breakdown.velocity > 0.9);
    }

    #[test]
    fn low_activity_decaying() {
        let h = calculate_signal_health(2, 1, 3.0, "declining", 7.0);
        assert!(h.score < 0.5);
        assert!(h.breakdown.activity < 0.1);
    }

    #[test]
    fn stable_mid_range() {
        let h = calculate_signal_health(15, 6, 6.0, "stable", 7.0);
        let s = h.score;
        assert!(s > 0.3 && s < 0.8);
        assert!((h.breakdown.velocity - 0.5).abs() < 0.01);
    }
}
