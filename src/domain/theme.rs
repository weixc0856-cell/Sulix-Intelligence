//! 主题领域模型
//!
//! 核心类型：Theme（原始主题）、ThemeAnalysis（分析结果）、
//! Assumption（承重假设）、AdverseScenario（逆境情景）、
//! CausalChain（因果链）、Summary（综合摘要）。

use serde::{Deserialize, Serialize};

use crate::domain::evidence::FactBaseEntry;
use crate::fetcher::Article;

/// 一个主题
#[derive(Debug, Clone, Serialize)]
pub struct Theme {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub articles: Vec<Article>,
    pub sources: Vec<String>,
}

/// 承重假设
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assumption {
    pub text: String,
    pub load_bearing: bool,
    pub evidence_strength: String,
}

/// 逆境情景
#[derive(Debug, Clone, Serialize)]
pub struct AdverseScenario {
    pub scenario: String,
    pub early_warning: String,
    pub severity: String,
}

/// 因果链
#[derive(Debug, Clone, Serialize)]
pub struct CausalChain {
    pub trigger: String,
    pub direct_effect: String,
    pub chain_reaction: Vec<String>,
    pub second_order: Vec<String>,
}

/// 主题分析结果
#[derive(Debug, Clone, Serialize)]
pub struct ThemeAnalysis {
    pub theme_id: String,
    pub theme_title: String,
    pub bluf: String,
    pub impact: String,
    pub geopolitical_fact: String,
    pub supply_chain_impact: String,
    pub analysis_paragraph: String,
    pub evidence_level: String,
    pub signal_strength: u8,
    pub fact_base: Vec<FactBaseEntry>,
    pub connections: Vec<String>,
    pub source_urls: Vec<String>,
    pub assumptions: Vec<Assumption>,
    pub adverse: Option<AdverseScenario>,
    pub next_tests: Vec<String>,
    pub open_questions: Vec<String>,
    pub chains: Vec<CausalChain>,
    pub what_to_do: String,
    pub what_to_watch: String,
}

/// 综合摘要
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub headline: String,
    pub narrative: String,
    pub total_articles: usize,
    pub theme_count: usize,
}
