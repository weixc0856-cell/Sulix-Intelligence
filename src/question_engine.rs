//! Question Engine — 将信号匹配到用户的声明式关切问题
//!
//! 输入: 已分析的主题 (ThemeAnalysis) + 用户的关切问题列表
//! 输出: 每个主题匹配到的问题列表 + 相关性评分
//!
//! Phase 2: 声明式配置，通过 config.toml 的 [questions] 段注入。
//! Code Review 认知去噪: 只有当该主题对问题提供了实质性(Substantial)的
//! 新数据或路径扭转时，相关性评分(Relevance)才允许 ≥ 7。

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 用户关切问题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    /// TOML 中为 "question" 字段
    #[serde(rename = "question")]
    pub text: String,
    #[serde(default)]
    pub category: String,
    #[serde(default = "default_priority")]
    pub priority: u8,
}

fn default_priority() -> u8 {
    5
}

/// 问题-主题匹配结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionMatch {
    pub question_id: String,
    pub question_text: String,
    /// 相关性评分 0-10。只有实质性新数据或路径扭转才允许 ≥ 7
    pub relevance: u8,
    /// 简要推理过程
    pub reasoning: String,
    /// 该匹配对决策的影响: Support / Challenge / Neutral
    pub evidence_type: String,
}

/// 将分析结果与用户关切问题进行匹配
///
/// Code Review 认知去噪:
/// Relevance ≥ 7 只有在提供了实质性新数据或路径扭转时才允许。
/// 防止 LLM "强行加戏"的泛化匹配。
pub fn match_questions(
    analysis: &crate::clusterer::ThemeAnalysis,
    questions: &[Question],
    _llm_client: &reqwest::Client,
    _api_key: &str,
    _llm_config: &crate::config::LlmConfig,
) -> Result<Vec<QuestionMatch>> {
    // Phase 2 基线实现: 基于关键词匹配的简单评分
    // Phase 3 将升级为 LLM 调用实现语义匹配
    let mut matches = Vec::new();
    let analysis_text = format!(
        "{} {} {} {}",
        analysis.bluf, analysis.impact, analysis.geopolitical_fact, analysis.supply_chain_impact
    )
    .to_lowercase();

    for question in questions {
        let question_lower = question.text.to_lowercase();
        let question_words: Vec<&str> = question_lower.split_whitespace().collect();

        // 计算关键词重叠
        let match_count = question_words
            .iter()
            .filter(|w| w.len() > 2 && analysis_text.contains(*w))
            .count();
        let total_keywords = question_words.iter().filter(|w| w.len() > 2).count();

        let relevance = if total_keywords == 0 {
            0
        } else {
            let ratio = match_count as f64 / total_keywords as f64;
            // Code Review: 仅在实质性匹配时给高分
            if ratio >= 0.6 {
                ((ratio * 10.0).round() as u8).min(10)
            } else if ratio >= 0.3 {
                ((ratio * 7.0).round() as u8).min(6)
            } else {
                0
            }
        };

        if relevance > 0 {
            matches.push(QuestionMatch {
                question_id: question.id.clone(),
                question_text: question.text.clone(),
                relevance,
                reasoning: format!("关键词匹配度: {}/{}", match_count, total_keywords),
                evidence_type: if relevance >= 7 {
                    "Support".into()
                } else if relevance >= 4 {
                    "Neutral".into()
                } else {
                    "Challenge".into()
                },
            });
        }
    }

    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clusterer::ThemeAnalysis;

    fn make_test_analysis() -> ThemeAnalysis {
        ThemeAnalysis {
            theme_id: "t1".into(),
            theme_title: "semiconductor export controls".into(),
            bluf: "US tightens advanced process export controls on China".into(),
            impact: "TSMC 3nm capacity allocation restricted".into(),
            geopolitical_fact: "BIS adds entities to export control list".into(),
            supply_chain_impact:
                "AI chip supply further constrained by semiconductor export limits".into(),
            analysis_paragraph: String::new(),
            evidence_level: String::new(),
            signal_strength: 7,
            fact_base: vec![],
            connections: vec![],
            source_urls: vec![],
            assumptions: vec![],
            adverse: None,
            next_tests: vec![],
            open_questions: vec![],
            chains: vec![],
        }
    }

    #[test]
    fn test_match_high_relevance() {
        let analysis = make_test_analysis();
        let questions = vec![Question {
            id: "q1".into(),
            text: "semiconductor export controls and advanced process".into(),
            category: "Tech".into(),
            priority: 8,
        }];
        let client = reqwest::Client::new();
        let config = crate::config::LlmConfig {
            api_key: Some("test".into()),
            provider: "deepseek".into(),
            model: "test".into(),
            base_url: "https://test.com".into(),
            max_tokens: 100,
            temperature: 0.1,
            perplexity_key: None,
        };
        let matches = match_questions(&analysis, &questions, &client, "test", &config).unwrap();
        assert!(!matches.is_empty(), "Should match semiconductor topic");
    }

    #[test]
    fn test_no_match_unrelated() {
        let analysis = make_test_analysis();
        let questions = vec![Question {
            id: "q2".into(),
            text: "federal reserve interest rates and inflation".into(),
            category: "Macro".into(),
            priority: 6,
        }];
        let client = reqwest::Client::new();
        let config = crate::config::LlmConfig {
            api_key: Some("test".into()),
            provider: "deepseek".into(),
            model: "test".into(),
            base_url: "https://test.com".into(),
            max_tokens: 100,
            temperature: 0.1,
            perplexity_key: None,
        };
        let matches = match_questions(&analysis, &questions, &client, "test", &config).unwrap();
        assert!(matches.is_empty(), "Unrelated question should not match");
    }
}
