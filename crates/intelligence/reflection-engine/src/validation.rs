//! Reflection validation — schema, grounding, and quality checks.
//!
//! Validates the LLM output before persistence.  Prevents empty, untraceable,
//! or low-quality reflections from entering the system.

use crate::generator::ReflectionDraft;

/// Result of validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub quality_score: f64,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Validate a ReflectionDraft against the spec contract.
///
/// Rules:
/// - result ∈ {correct, wrong, mixed}
/// - lessons ≥ 1
/// - each description ≥ 20 chars
/// - confidence ∈ [0.0, 1.0]
/// - each lesson has evidence_basis.length > 0
/// - each rule has action_type + action_instruction
pub fn validate(draft: &ReflectionDraft) -> ValidationResult {
    let mut errors: Vec<String> = Vec::new();
    let warnings: Vec<String> = Vec::new();

    // 1. Result
    match draft.result.as_str() {
        "correct" | "wrong" | "mixed" => {}
        _ => errors.push(format!("invalid result: {}", draft.result)),
    }

    // 2. Lessons ≥ 1
    if draft.lessons.is_empty() {
        errors.push("at least 1 lesson required".into());
    }

    for (i, lesson) in draft.lessons.iter().enumerate() {
        // 3. Description ≥ 20 chars
        if lesson.description.len() < 20 {
            errors.push(format!("lesson {}: description too short ({} chars)", i, lesson.description.len()));
        }
        // 4. Confidence
        if !(0.0..=1.0).contains(&lesson.confidence) {
            errors.push(format!("lesson {}: confidence out of range [0,1]: {}", i, lesson.confidence));
        }
        // 5. Evidence grounding
        if lesson.evidence_basis.is_empty() {
            errors.push(format!("lesson {}: evidence_basis is empty (must be traceable)", i));
        }
    }

    // 6. Rules sanity
    for (i, rule) in draft.rules.iter().enumerate() {
        if rule.action_type.is_empty() {
            errors.push(format!("rule {}: action_type is required", i));
        }
        if rule.action_instruction.is_empty() {
            errors.push(format!("rule {}: action_instruction is required", i));
        }
        if !(0.0..=1.0).contains(&rule.confidence) {
            errors.push(format!("rule {}: confidence out of range [0,1]: {}", i, rule.confidence));
        }
    }

    let quality_score = draft.quality_score.clamp(0.0, 1.0);

    ValidationResult {
        valid: errors.is_empty(),
        quality_score,
        errors,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_draft_passes() {
        let draft = ReflectionDraft {
            result: "wrong".into(),
            confidence_calibration: "overestimated".into(),
            quality_score: 0.85,
            lessons: vec![crate::generator::LessonDraft {
                category: "assumption_error".into(),
                domain: "investment".into(),
                description: "技术突破不等于商业采用，低估了客户教育成本".into(),
                severity: "high".into(),
                confidence: 0.9,
                evidence_basis: vec!["OUT-001".into()],
            }],
            rules: vec![crate::generator::RuleDraft {
                condition_domain: "investment".into(),
                condition_trigger: "AI startup evaluation".into(),
                action_type: "require_validation".into(),
                action_instruction: "verify paid customer adoption".into(),
                confidence: 0.85,
            }],
        };
        let result = validate(&draft);
        assert!(result.valid, "errors: {:?}", result.errors);
        assert!((result.quality_score - 0.85).abs() < 0.01);
    }

    #[test]
    fn empty_lessons_fails() {
        let draft = ReflectionDraft {
            result: "correct".into(),
            confidence_calibration: "accurate".into(),
            quality_score: 0.5,
            lessons: vec![],
            rules: vec![],
        };
        assert!(!validate(&draft).valid);
    }

    #[test]
    fn missing_evidence_fails() {
        let draft = ReflectionDraft {
            result: "wrong".into(),
            confidence_calibration: "overestimated".into(),
            quality_score: 0.5,
            lessons: vec![crate::generator::LessonDraft {
                category: "test".into(),
                domain: "test".into(),
                description: "this is a lesson without any evidence at all".into(),
                severity: "low".into(),
                confidence: 0.5,
                evidence_basis: vec![],
            }],
            rules: vec![],
        };
        assert!(!validate(&draft).valid);
    }
}
