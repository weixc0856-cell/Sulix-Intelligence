//! Publishing helpers — shared utilities for all stages

use crate::domain::theme::{Theme, ThemeAnalysis};

/// Research Agent 的输出（传递给 Publishing Agent）
pub struct ResearchOutput {
    pub themes: Vec<Theme>,
    pub analyses: Vec<ThemeAnalysis>,
    pub analyses_zh: Vec<ThemeAnalysis>,
    pub triage: crate::agent::scan::TriageResult,
    pub new_articles: Vec<crate::fetcher::Article>,
}

impl ResearchOutput {
    #[allow(clippy::type_complexity)]
    pub fn destructure(self) -> (Vec<Theme>, Vec<ThemeAnalysis>, Vec<ThemeAnalysis>, Vec<crate::fetcher::Article>, crate::agent::scan::TriageResult) {
        (self.themes, self.analyses, self.analyses_zh, self.new_articles, self.triage)
    }
}

/// Extract entities from analysis fact_base (deduplicated entity list per analysis)
pub fn extract_entities(analysis: &ThemeAnalysis) -> Vec<String> {
    let known = crate::entity::known_entities();
    let mut entities = Vec::new();
    for fb in &analysis.fact_base {
        for word in fb.evidence.split_whitespace() {
            let upper = word.to_uppercase();
            if known.contains(&upper.as_str()) && !entities.contains(&upper) {
                entities.push(upper);
            }
        }
    }
    entities
}

/// 主题分析 + 蓝军验证辅助函数
/// 由 main.rs 的 agent_research 调用
pub async fn analyze_and_validate(
    theme: &Theme,
    api_key: &str,
    llm_config: &crate::config::LlmConfig,
    prompts: Option<&crate::config::PromptsConfig>,
    language: &str,
) -> Option<ThemeAnalysis> {
    let mut analysis = match crate::engine::analysis::analyze_theme(theme, api_key, llm_config, language, prompts).await {
        Ok(a) => a,
        Err(e) => {
            log::warn!("⚠️ 主题分析失败 [{}|{}]: {}", language, theme.title, e);
            return None;
        }
    };
    match crate::engine::analysis::challenge_theme(&analysis, api_key, llm_config, prompts).await {
        Ok((assumptions, adverse, next_tests, open_questions)) => {
            analysis.assumptions = assumptions;
            analysis.adverse = adverse;
            analysis.next_tests = next_tests;
            analysis.open_questions = open_questions;
        }
        Err(e) => log::warn!("⚠️ 蓝军验证失败 [{}|{}], 使用无蓝军分析: {}", language, theme.title, e),
    }
    Some(analysis)
}
