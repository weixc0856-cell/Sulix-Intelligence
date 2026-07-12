//! 数据类型 — LLM 输入/输出结构体

use serde::{Deserialize, Serialize};

/// 单个 vertical 的分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerticalAnalysis {
    pub category: String,
    pub articles: Vec<AnalyzedArticle>,
}

/// 分析后的文章（支持红蓝对抗：strategic_level=S/A/B/C）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedArticle {
    pub title: String,
    pub url: String,
    pub importance: u8,
    pub relevance: String,
    pub time_horizon: String,
    pub action: String,
    pub confidence: String,
    pub judgment: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub strategic_level: String,
    #[serde(default)]
    pub blue_rebuttal: String,
    #[serde(default)]
    pub arbitration: String,
    #[serde(default)]
    pub evidence_type: String,
}

/// Raw LLM response struct — only fields consumed downstream.
#[derive(Debug, Deserialize)]
pub struct AnalyzedArticleRaw {
    pub title: String,
    pub importance: u8,
    pub relevance: String,
    pub judgment: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ChoiceMessage,
}

#[derive(Debug, Deserialize)]
pub struct ChoiceMessage {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ArticlesWrapper {
    pub articles: Vec<AnalyzedArticleRaw>,
}
