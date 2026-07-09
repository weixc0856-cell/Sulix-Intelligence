//! Schematized Assessment — 规范评估对象的验证定义

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use crate::domain::Localized;

/// 规范评估对象（验证 Schema 用）
/// 对应 frontend contracts/assessment.schema.json
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssessmentObject {
    pub id: String,
    /// 三语言标题
    pub title: Localized,
    pub date: String,
    pub status: String,
    pub confidence: f64,
    #[serde(default)]
    pub evidences: i32,
    #[serde(default)]
    pub challenges: i32,
    /// 三语言摘要
    pub summary: Option<Localized>,
    #[serde(default)]
    pub decision: Option<String>,
    /// 三语言决策依据
    #[serde(default)]
    pub decision_rationale: Option<Localized>,
    #[serde(default)]
    pub supporting_evidence: Vec<String>,
    #[serde(default)]
    pub conflicting_evidence: Vec<String>,
    // TODO: upgrade supporting_evidence/conflicting_evidence to Vec<Localized>
    // when Evidence struct is created (currently string-only)
    #[serde(default = "crate::schema::validator::default_locale")]
    pub locale: String,
    /// 原文语言: "en" | "zh-cn" | "zh-tw"
    #[serde(default = "crate::schema::validator::default_lang")]
    pub lang: String,
}

impl AssessmentObject {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.id.is_empty() { errors.push("id: empty".into()); }
        if self.title.is_empty() { errors.push("title: empty".into()); }
        if self.date.is_empty() { errors.push("date: empty".into()); }
        if self.status.is_empty() { errors.push("status: empty".into()); }
        if self.confidence < 0.0 || self.confidence > 1.0 {
            errors.push("confidence: out of range [0,1]".into());
        }
        if !["en", "zh-cn", "zh-tw"].contains(&self.lang.as_str()) {
            errors.push(format!("lang: invalid '{}'", self.lang));
        }

        // Phase 0: warn-only for empty evidence
        if self.evidences == 0 && self.supporting_evidence.is_empty() {
            errors.push("evidence: empty (Phase 0)".into());
        }

        errors
    }
}

impl crate::schema::validator::Validate for AssessmentObject {
    fn object_type() -> &'static str { "assessment" }
    fn object_id(&self) -> &str { &self.id }
    fn validate(&self) -> Vec<String> { self.validate() }
}
