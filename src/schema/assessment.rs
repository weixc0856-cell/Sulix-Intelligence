//! Schematized Assessment — 规范评估对象的验证定义

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// 规范评估对象（验证 Schema 用）
/// 对应 frontend contracts/assessment.schema.json
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssessmentObject {
    pub id: String,
    pub title: String,
    pub date: String,
    pub status: String,
    pub confidence: f64,
    #[serde(default)]
    pub evidences: i32,
    #[serde(default)]
    pub challenges: i32,
    pub summary: Option<String>,
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub decision_rationale: Option<String>,
    #[serde(default)]
    pub supporting_evidence: Vec<String>,
    #[serde(default)]
    pub conflicting_evidence: Vec<String>,
    #[serde(default = "default_locale")]
    pub locale: String,
}

fn default_locale() -> String { "en".into() }

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

        // Phase 0: warn-only for empty evidence
        if self.evidences == 0 && self.supporting_evidence.is_empty() {
            errors.push("evidence: empty (Phase 0)".into());
        }

        errors
    }
}
