//! Impact scoring — maps raw signal scores to human-readable impact levels.

/// Convert a normalised signal score (0.0–1.0) to an impact label.
///
/// Thresholds:
/// - `>= 0.7` → High
/// - `>= 0.4` → Medium
/// - `< 0.4`  → Low
pub fn score_to_impact(score: f64) -> &'static str {
    if score >= 0.7 {
        "High"
    } else if score >= 0.4 {
        "Medium"
    } else {
        "Low"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_impact() {
        assert_eq!(score_to_impact(0.7), "High");
        assert_eq!(score_to_impact(1.0), "High");
        assert_eq!(score_to_impact(0.85), "High");
    }

    #[test]
    fn medium_impact() {
        assert_eq!(score_to_impact(0.4), "Medium");
        assert_eq!(score_to_impact(0.69), "Medium");
        assert_eq!(score_to_impact(0.5), "Medium");
    }

    #[test]
    fn low_impact() {
        assert_eq!(score_to_impact(0.39), "Low");
        assert_eq!(score_to_impact(0.0), "Low");
        assert_eq!(score_to_impact(0.1), "Low");
    }
}
