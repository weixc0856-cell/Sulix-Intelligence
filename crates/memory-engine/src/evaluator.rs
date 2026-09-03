use crate::model::PromotionScore;

#[derive(Debug, Clone, PartialEq)]
pub enum EvaluationResult {
    Promote { score: PromotionScore },
    Review { score: PromotionScore },
    Archive { reason: String },
}

pub fn check_gate(quality_score: f64, has_outcome: bool, has_evidence: bool, has_lessons_or_rules: bool) -> Option<()> {
    if quality_score < 0.7 {
        return None;
    }
    if !has_outcome {
        return None;
    }
    if !has_evidence {
        return None;
    }
    if !has_lessons_or_rules {
        return None;
    }
    Some(())
}

pub fn calculate_score(confidence: f32, recurrence: f32, impact: f32, evidence: f32, stability: f32) -> PromotionScore {
    let total = 0.25 * confidence + 0.20 * recurrence + 0.20 * impact + 0.20 * evidence + 0.15 * stability;
    PromotionScore { confidence, recurrence, impact, evidence, stability, total }
}

pub fn evaluate(
    quality_score: f64,
    has_outcome: bool,
    has_evidence: bool,
    has_lessons_or_rules: bool,
    recurrence: f32,
    impact: f32,
    stability: f32,
) -> EvaluationResult {
    match check_gate(quality_score, has_outcome, has_evidence, has_lessons_or_rules) {
        Some(()) => {
            let score = calculate_score(quality_score as f32, recurrence, impact, 0.8, stability);
            if score.total > 0.75 {
                EvaluationResult::Promote { score }
            } else if score.total >= 0.4 {
                EvaluationResult::Review { score }
            } else {
                EvaluationResult::Archive { reason: format!("score too low: {:.2}", score.total) }
            }
        }
        None => EvaluationResult::Archive { reason: "promotion gate failed".into() },
    }
}

pub fn effective_confidence(confidence: f64, days_since: i64, lambda: f64) -> f64 {
    if days_since <= 0 {
        return confidence;
    }
    confidence * (-lambda * days_since as f64).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_gate_passes() {
        assert!(check_gate(0.8, true, true, true).is_some());
    }
    #[test]
    fn promotion_gate_fails_low_quality() {
        assert!(check_gate(0.5, true, true, true).is_none());
    }
    #[test]
    fn promotion_gate_fails_no_outcome() {
        assert!(check_gate(0.8, false, true, true).is_none());
    }
    #[test]
    fn score_calculation() {
        let s = calculate_score(0.9, 0.5, 0.6, 0.7, 0.8);
        let expected = 0.25 * 0.9 + 0.20 * 0.5 + 0.20 * 0.6 + 0.20 * 0.7 + 0.15 * 0.8;
        assert!((s.total - expected as f32).abs() < 0.01);
    }
    #[test]
    fn evaluate_promotes_high_score() {
        let r = evaluate(0.85, true, true, true, 0.8, 0.7, 0.7);
        assert!(matches!(r, EvaluationResult::Promote { .. }));
    }
    #[test]
    fn evaluate_archives_low_quality() {
        let r = evaluate(0.5, true, true, true, 0.5, 0.5, 0.5);
        assert!(matches!(r, EvaluationResult::Archive { .. }));
    }
    #[test]
    fn confidence_decay_over_time() {
        let e = effective_confidence(0.9, 250, 0.002);
        assert!(e < 0.9 && e > 0.5);
    }
    #[test]
    fn confidence_decay_zero_days() {
        let e = effective_confidence(0.9, 0, 0.002);
        assert!((e - 0.9).abs() < 0.001);
    }
}
