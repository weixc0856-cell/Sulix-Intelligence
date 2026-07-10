//! Schematized Signal — 规范信号对象的验证定义

use crate::domain::Localized;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 规范信号对象（验证 Schema 用）
/// 对应 frontend contracts/signal.schema.json
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SignalObject {
    pub id: String,
    /// 三语言标题
    pub title: Localized,
    pub date: String,
    pub svi: f64,
    pub asi: f64,
    pub confidence: f64,
    pub source: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub entities: Vec<String>,
    /// 三语言摘要（可选，仅标题翻翻，摘要由分级政策定）
    #[serde(default)]
    pub summary: Option<Localized>,
    #[serde(default = "crate::schema::validator::default_locale")]
    pub locale: String,
    /// 原文语言: "en" | "zh-cn" | "zh-tw"
    #[serde(default = "crate::schema::validator::default_lang")]
    pub lang: String,
}

impl SignalObject {
    /// 验证必填字段
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.id.is_empty() {
            errors.push("id: empty".into());
        }
        if self.title.is_empty() {
            errors.push("title: empty".into());
        }
        if self.date.is_empty() {
            errors.push("date: empty".into());
        }
        if !(0.0..=10.0).contains(&self.svi) {
            errors.push("svi: out of range [0,10]".into());
        }
        if self.confidence < 0.0 || self.confidence > 1.0 {
            errors.push("confidence: out of range [0,1]".into());
        }
        if self.source.is_empty() {
            errors.push("source: empty".into());
        }
        if !["en", "zh-cn", "zh-tw"].contains(&self.lang.as_str()) {
            errors.push(format!("lang: invalid '{}'", self.lang));
        }

        errors
    }
}

impl crate::schema::validator::Validate for SignalObject {
    fn object_type() -> &'static str {
        "signal"
    }
    fn object_id(&self) -> &str {
        &self.id
    }
    fn validate(&self) -> Vec<String> {
        self.validate()
    }
}
