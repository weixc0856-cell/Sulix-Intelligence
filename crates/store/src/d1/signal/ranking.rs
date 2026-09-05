//! Radar ranking formula.
//!
//! RadarScore = 0.5 * Health + 0.3 * Recency + 0.2 * Novelty
//!
//!   Health   — signal health score (long-term strength)
//!   Recency  — how recently the last evidence appeared (0-1, higher = more recent)
//!   Novelty  — how new this signal is (0-1, higher = new discovery)
//!
//! Pure function, no DB access.

/// Compute radar ranking score for a signal.
pub fn radar_score(health: f64, last_evidence_ago_secs: f64, first_seen_ago_secs: f64) -> f64 {
    // Recency: last evidence within 24h → 1.0, 7 days → 0.0
    let recency = 1.0 - (last_evidence_ago_secs / 604800.0).clamp(0.0, 1.0);

    // Novelty: first seen within 48h → 1.0, 14 days → 0.0
    let novelty = 1.0 - (first_seen_ago_secs / 1209600.0).clamp(0.0, 1.0);

    0.5 * health + 0.3 * recency + 0.2 * novelty
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_signal_scores_high() {
        let s = radar_score(0.8, 3600.0, 7200.0);
        assert!(s > 0.7);
    }

    #[test]
    fn old_signal_scores_lower() {
        let s = radar_score(0.5, 604800.0 * 7.0, 1209600.0 * 14.0);
        assert!(s < 0.4);
    }

    #[test]
    fn novelty_boosts_new_signals() {
        let new_signal = radar_score(0.6, 3600.0, 7200.0);
        let old_signal = radar_score(0.6, 3600.0, 1209600.0 * 30.0);
        assert!(new_signal > old_signal);
    }
}
