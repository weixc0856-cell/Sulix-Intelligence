//! Schema Object 映射器 — domain type → schema object 纯转换
//!
//! 职责：将引擎内部的 domain 类型映射为 schema 验证对象。
//! 纪律：纯转换，零兜底。缺失字段不补 default/fallback，validator 据此报错。

use crate::domain::compute_confidence;
use crate::domain::evidence::Stance;
use crate::domain::thesis::Thesis;
use crate::domain::DecisionRecord;
use crate::domain::Localized;
use crate::domain::ThesisDecision;
use crate::schema::assessment::AssessmentObject;
use crate::schema::decision::DecisionObject;

/// Thesis + 可选 ThesisDecision → AssessmentObject
pub fn thesis_to_assessment(
    thesis: &Thesis,
    decision: Option<&ThesisDecision>,
    locale: &str,
) -> AssessmentObject {
    AssessmentObject {
        id: thesis.assessment_id.clone().unwrap_or_default(),
        title: Localized::en_only(&thesis.title),
        date: thesis.updated.clone(),
        status: format!("{:?}", thesis.status).to_lowercase(),
        confidence: compute_confidence(&thesis.evidences),
        evidences: thesis
            .evidences
            .iter()
            .filter(|e| e.stance == Stance::Supports)
            .count() as i32,
        challenges: thesis
            .evidences
            .iter()
            .filter(|e| e.stance == Stance::Challenges)
            .count() as i32,
        summary: None,
        decision: decision.map(|d| d.decision_type.as_key().to_string()),
        decision_rationale: decision.map(|d| Localized::en_only(&d.rationale)),
        supporting_evidence: vec![],
        conflicting_evidence: vec![],
        locale: locale.to_string(),
        lang: "en".to_string(),
    }
}

/// DecisionRecord → DecisionObject
pub fn decision_record_to_object(record: &DecisionRecord, locale: &str) -> DecisionObject {
    DecisionObject {
        id: record.id.clone(),
        title: Localized::en_only(&format!("{} — {}", record.id, record.decision_type)),
        decision_type: record.decision_type.clone(),
        confidence: record.confidence,
        horizon: record.horizon.clone(),
        asm_id: Some(record.asm_id.clone()),
        rationale: Some(Localized::en_only(&record.rationale)),
        risk: None,
        stability: Some(record.stability.clone()),
        state: Some(format!("{:?}", record.state).to_lowercase()),
        primary_domain: Some(record.primary_domain.label().to_string()),
        locale: locale.to_string(),
        lang: "en".to_string(),
    }
}
