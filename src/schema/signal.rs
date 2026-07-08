//! Schematized Signal — 规范信号对象的验证定义

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// 规范信号对象（验证 Schema 用）
/// 对应 frontend contracts/signal.schema.json
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SignalObject {
    pub id: String,
    pub title: String,
    pub date: String,
    pub svi: f64,
    pub asi: f64,
    pub confidence: f64,
    pub source: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub entities: Vec<String>,
    pub summary: Option<String>,
    #[serde(default = "default_locale")]
    pub locale: String,
}

fn default_locale() -> String { "en".into() }

impl SignalObject {
    /// 验证必填字段
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.id.is_empty() { errors.push("id: empty".into()); }
        if self.title.is_empty() { errors.push("title: empty".into()); }
        if self.date.is_empty() { errors.push("date: empty".into()); }
        if !(0.0..=10.0).contains(&self.svi) { errors.push("svi: out of range [0,10]".into()); }
        if self.confidence < 0.0 || self.confidence > 1.0 { errors.push("confidence: out of range [0,1]".into()); }
        if self.source.is_empty() { errors.push("source: empty".into()); }

        errors
    }
}
