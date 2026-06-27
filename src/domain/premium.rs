use serde::{Deserialize, Serialize};

/// 最终 Premium 研报
#[derive(Debug, Clone, Serialize)]
pub struct PremiumReport {
    pub theme_title: String,
    pub date: String,
    pub executive_summary: String,
    pub geopolitical_assessment: String,
    pub technical_impact: String,
    pub commercial_framework: String,
    pub risk_scenarios: Vec<String>,
    pub sources: Vec<String>,
    /// Research stage: "what-changed" | "why-it-matters" | "what-to-do"
    pub stage: String,
    /// Whether this is a premium (vs. quick) research report
    pub is_premium: bool,
}

/// 专题聚合/紧急加更配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialTopic {
    pub topic_id: String,
    pub title: String,
    pub is_flash: bool,
    pub perspective: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub locked_sources: Option<Vec<String>>,
}
