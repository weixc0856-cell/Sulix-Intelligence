//! Schematized Decision — 规范决策对象的验证定义

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// 规范决策对象（验证 Schema 用）
/// 对应 frontend contracts/decision.schema.json
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DecisionObject {
    pub id: String,
    pub title: String,
    pub decision_type: String,
    pub confidence: f64,
    pub horizon: String,
    pub asm_id: Option<String>,
    pub rationale: Option<String>,
    pub risk: Option<String>,
    pub stability: Option<String>,
    pub state: Option<String>,
    pub primary_domain: Option<String>,
    #[serde(default = "default_locale")]
    pub locale: String,
}

fn default_locale() -> String { "en".into() }

impl DecisionObject {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.id.is_empty() { errors.push("id: empty".into()); }
        if self.title.is_empty() { errors.push("title: empty".into()); }
        if self.decision_type.is_empty() { errors.push("decision_type: empty".into()); }
        if self.horizon.is_empty() { errors.push("horizon: empty".into()); }
        if self.confidence < 0.0 || self.confidence > 1.0 {
            errors.push("confidence: out of range [0,1]".into());
        }

        // 有效 decision_type
        let valid_types = ["build", "invest", "monitor", "learn", "ignore", "exit"];
        if !valid_types.contains(&self.decision_type.as_str()) {
            errors.push(format!("decision_type: invalid '{}'", self.decision_type));
        }

        errors
    }
}
