//! Pure scoring functions for entity signal candidates — no D1 dependency.
//!
//! These are extracted from the repeating inline logic in `candidate.rs`.
//! All functions are deterministic and testable without wasm32.

/// Compute trend from recent (3d) vs earlier (3-6d) article counts.
///
/// Returns `"rising"`, `"declining"`, or `"stable"`.
pub fn compute_trend(recent_count: i64, earlier_count: i64) -> &'static str {
    if earlier_count == 0 || recent_count > earlier_count * 12 / 10 {
        "rising"
    } else if recent_count < earlier_count * 8 / 10 {
        "declining"
    } else {
        "stable"
    }
}

/// Compute novelty as the ratio of current rate to historical rate.
///
/// `current_rate` = articles / days
/// `historical_rate` = articles / 21 days (or 50% of current rate if no history)
/// Returns a value clamped to [0.0, 1.0].
pub fn compute_novelty(current_rate: f64, historical_count: i64, _days: i64) -> f64 {
    let historical_rate = if historical_count > 0 { historical_count as f64 / 21.0 } else { current_rate * 0.5 };
    let novelty_raw = if historical_rate > 0.0 { current_rate / historical_rate } else { 1.0 };
    (novelty_raw / 3.0).min(1.0)
}

/// Results of the 5-factor scoring computation.
pub struct SignalScores {
    pub score: f64,
    pub volume: f64,
    pub diversity: f64,
    pub quality: f64,
    pub velocity: f64,
    pub novelty: f64,
}

/// Compute the 5-factor signal score from raw metrics.
///
/// Factors:
///   volume     (0.25) — articles per day, capped at 20, normalised
///   diversity  (0.20) — unique sources, capped at 10
///   quality    (0.20) — avg_score / 10.0
///   velocity   (0.20) — trend-based (rising=1, stable=0.5, declining=0)
///   novelty    (0.15) — current rate vs historical rate
pub fn compute_5_factor_score(
    article_count: i64,
    source_count: i64,
    avg_score: f64,
    trend: &str,
    current_rate: f64,
    historical_count: i64,
    days: i64,
) -> SignalScores {
    let volume = (article_count as f64 / days as f64).min(20.0) / 20.0;
    let diversity = (source_count as f64).min(10.0) / 10.0;
    let quality = (avg_score / 10.0).clamp(0.0, 1.0);
    let velocity = match trend {
        "rising" => 1.0,
        "stable" => 0.5,
        _ => 0.0,
    };
    let novelty = compute_novelty(current_rate, historical_count, days);

    let score = 0.25 * volume + 0.20 * diversity + 0.20 * quality + 0.20 * velocity + 0.15 * novelty;

    SignalScores { score, volume, diversity, quality, velocity, novelty }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trend_rising() {
        assert_eq!(compute_trend(15, 10), "rising"); // 15 > 12
        assert_eq!(compute_trend(13, 10), "rising"); // 13 > 12
    }

    #[test]
    fn test_trend_declining() {
        assert_eq!(compute_trend(7, 10), "declining"); // 7 < 8
    }

    #[test]
    fn test_trend_stable() {
        assert_eq!(compute_trend(10, 10), "stable");
        assert_eq!(compute_trend(11, 10), "stable");
        assert_eq!(compute_trend(0, 0), "rising"); // earlier=0 → rising
    }

    #[test]
    fn test_novelty_new_signal() {
        let n = compute_novelty(10.0, 0, 7);
        assert!(n > 0.0 && n <= 1.0);
    }

    #[test]
    fn test_novelty_historical() {
        // current_rate = 10/7 ≈ 1.43, hist_rate = 30/21 ≈ 1.43 → raw ≈ 1.0 → clamped ≈ 0.33
        let n = compute_novelty(1.43, 30, 7);
        assert!((n - 0.33).abs() < 0.02, "expected ~0.33, got {n}");
    }

    #[test]
    fn test_5_factor_high_signal() {
        // 200 articles / 7 = 28.6 daily → volume capped at 20 → 20/20 = 1.0
        let s = compute_5_factor_score(200, 10, 8.5, "rising", 28.57, 10, 7);
        assert!(s.score > 0.5);
        assert!(s.volume > 0.9, "volume should be ~1.0, got {}", s.volume);
        assert!(s.velocity > 0.9);
    }

    #[test]
    fn test_5_factor_low_signal() {
        let s = compute_5_factor_score(2, 1, 3.0, "declining", 0.29, 5, 7);
        assert!(s.score < 0.4);
        assert!(s.volume < 0.1);
    }
}
