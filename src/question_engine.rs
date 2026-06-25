//! Question Engine — 将信号匹配到用户的声明式关切问题
//!
//! ⚠️ Phase 1 冻结状态：代码保留但标记弃用以防误用。
//! 未来可能演变为 Research Planner / Hypothesis Generator / Investigation Generator 的雏形。
//!
//! 输入: 已分析的主题 (ThemeAnalysis) + 用户的关切问题列表
//! 输出: 每个主题匹配到的问题列表 + 相关性评分
//!
//! Phase 2: 声明式配置，通过 config.toml 的 [questions] 段注入。
//! Phase 3: 关键词匹配为主，LLM 语义匹配为后备。
//!
//! Code Review 认知去噪: 只有当该主题对问题提供了实质性(Substantial)的
//! 新数据或路径扭转时，相关性评分(Relevance)才允许 ≥ 7。

#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::clusterer::ThemeAnalysis;
use crate::config::LlmConfig;
use crate::llm;

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

/// Question Engine 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionEngineConfig {
    /// 是否启用 LLM 语义匹配（作为关键词匹配的后备）
    #[serde(default = "default_semantic_matching")]
    pub enable_semantic_matching: bool,
}

impl Default for QuestionEngineConfig {
    fn default() -> Self {
        Self {
            enable_semantic_matching: default_semantic_matching(),
        }
    }
}

fn default_semantic_matching() -> bool {
    false
}

/// 关键词匹配：计算问题关键词在分析文本中的重叠比率
fn keyword_match(analysis_text: &str, question: &Question) -> (u8, String) {
    let question_lower = question.text.to_lowercase();
    let question_words: Vec<&str> = question_lower.split_whitespace().collect();
    let analysis_lower = analysis_text.to_lowercase();

    let match_count = question_words
        .iter()
        .filter(|w| w.len() > 2 && analysis_lower.contains(*w))
        .count();
    let total_keywords = question_words.iter().filter(|w| w.len() > 2).count();

    if total_keywords == 0 {
        return (0, "无关键词可匹配".into());
    }

    let ratio = match_count as f64 / total_keywords as f64;
    let relevance = if ratio >= 0.6 {
        ((ratio * 10.0).round() as u8).min(10)
    } else if ratio >= 0.3 {
        ((ratio * 7.0).round() as u8).min(6)
    } else {
        0
    };

    let reasoning = format!(
        "关键词匹配度: {}/{} (ratio={:.2})",
        match_count, total_keywords, ratio
    );
    (relevance, reasoning)
}

/// LLM 语义匹配：当关键词匹配无法确认时，调用 LLM 判断相关性
async fn llm_semantic_match(
    analysis: &ThemeAnalysis,
    question: &Question,
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<Option<(u8, String)>> {
    let system_prompt =
        "You are a relevance judge. Given a user's concern question and a news analysis, determine:
1. Whether the analysis provides SUBSTANTIAL new information relevant to the question (yes/no)
2. If yes, rate relevance 1-10
3. The evidence type: Support (supports the premise), Challenge (challenges it), or Neutral

Output JSON only:
{
  \"is_relevant\": true/false,
  \"relevance\": 7,
  \"evidence_type\": \"Support\",
  \"reasoning\": \"The analysis provides concrete data on export controls affecting the premise.\"
}";

    let user_prompt = format!(
        "## User Question\n{}\n\n## Analysis\nBLUF: {}\nImpact: {}\nGeopolitical: {}\nSupply Chain: {}\nSignal Strength: {}",
        question.text,
        analysis.bluf,
        analysis.impact,
        analysis.geopolitical_fact,
        analysis.supply_chain_impact,
        analysis.signal_strength,
    );

    let raw =
        llm::call_with_retry_raw(client, api_key, llm_config, system_prompt, &user_prompt).await?;
    let parsed: serde_json::Value = llm::parse_json_lenient(&raw)?;

    let is_relevant = parsed["is_relevant"].as_bool().unwrap_or(false);
    if !is_relevant {
        return Ok(None);
    }

    let relevance = parsed["relevance"].as_u64().unwrap_or(3).min(10) as u8;
    let _evidence_type = parsed["evidence_type"]
        .as_str()
        .unwrap_or("Neutral")
        .to_string();
    let reasoning = parsed["reasoning"]
        .as_str()
        .unwrap_or("LLM 语义匹配")
        .to_string();

    Ok(Some((relevance, format!("LLM: {}", reasoning))))
}

/// 将分析结果与用户关切问题进行匹配
///
/// 策略：关键词匹配为主（快速），LLM 语义匹配为后备（精准）。
/// 当关键词匹配结果处于模糊区间（relevance < 4）且开启了语义匹配时，
/// 触发 LLM 重新评估。
pub async fn match_questions(
    analysis: &ThemeAnalysis,
    questions: &[Question],
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    engine_config: &QuestionEngineConfig,
) -> Result<Vec<QuestionMatch>> {
    let analysis_text = format!(
        "{} {} {} {}",
        analysis.bluf, analysis.impact, analysis.geopolitical_fact, analysis.supply_chain_impact
    );

    let mut matches = Vec::new();

    for question in questions {
        // 先做关键词匹配（无延迟）
        let (kw_relevance, kw_reasoning) = keyword_match(&analysis_text, question);

        // 如果关键词匹配足够好，直接使用
        if kw_relevance >= 5 {
            matches.push(QuestionMatch {
                question_id: question.id.clone(),
                question_text: question.text.clone(),
                relevance: kw_relevance,
                reasoning: kw_reasoning,
                evidence_type: if kw_relevance >= 7 {
                    "Support".into()
                } else if kw_relevance >= 4 {
                    "Neutral".into()
                } else {
                    "Challenge".into()
                },
            });
            continue;
        }

        // 如果关键词匹配模糊（1-4）且开启了语义匹配，尝试 LLM
        if engine_config.enable_semantic_matching && kw_relevance > 0 {
            match llm_semantic_match(analysis, question, client, api_key, llm_config).await {
                Ok(Some((llm_relevance, llm_reasoning))) => {
                    matches.push(QuestionMatch {
                        question_id: question.id.clone(),
                        question_text: question.text.clone(),
                        relevance: llm_relevance.max(kw_relevance),
                        reasoning: llm_reasoning,
                        evidence_type: if llm_relevance >= 7 {
                            "Support".into()
                        } else if llm_relevance >= 4 {
                            "Neutral".into()
                        } else {
                            "Challenge".into()
                        },
                    });
                    continue;
                }
                Ok(None) => {
                    // LLM 也认为不相关 → 跳过
                    log::debug!(
                        "LLM: question '{}' not relevant to '{}'",
                        question.text,
                        analysis.theme_title
                    );
                }
                Err(e) => {
                    log::warn!(
                        "⚠️ LLM semantic match failed for '{}': {}",
                        question.text,
                        e
                    );
                }
            }
        }

        // 关键词匹配差 + (LLM 未启用 / LLM 失败) → 仍然用关键词结果（如 > 0）
        if kw_relevance > 0 {
            matches.push(QuestionMatch {
                question_id: question.id.clone(),
                question_text: question.text.clone(),
                relevance: kw_relevance,
                reasoning: kw_reasoning,
                evidence_type: "Neutral".into(),
            });
        }
    }

    Ok(matches)
}

/// 同步版本的 match_questions（用于旧调用点，内部包 async）
pub fn match_questions_sync(
    analysis: &ThemeAnalysis,
    questions: &[Question],
    _client: &reqwest::Client,
    _api_key: &str,
    _llm_config: &LlmConfig,
) -> Result<Vec<QuestionMatch>> {
    // 降级为纯关键词匹配（同步版本不需要 LLM）
    let analysis_text = format!(
        "{} {} {} {}",
        analysis.bluf, analysis.impact, analysis.geopolitical_fact, analysis.supply_chain_impact
    );

    let mut matches = Vec::new();
    for question in questions {
        let (relevance, reasoning) = keyword_match(&analysis_text, question);
        if relevance > 0 {
            matches.push(QuestionMatch {
                question_id: question.id.clone(),
                question_text: question.text.clone(),
                relevance,
                reasoning,
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
            what_to_do: String::new(),
            what_to_watch: String::new(),
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
        let matches = match_questions_sync(
            &analysis,
            &questions,
            &reqwest::Client::new(),
            "test",
            &crate::config::LlmConfig {
                api_key: Some("test".into()),
                provider: "deepseek".into(),
                model: "test".into(),
                base_url: "https://test.com".into(),
                max_tokens: 100,
                temperature: 0.1,
                perplexity_key: None,
            },
        )
        .unwrap();
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
        let matches = match_questions_sync(
            &analysis,
            &questions,
            &reqwest::Client::new(),
            "test",
            &crate::config::LlmConfig {
                api_key: Some("test".into()),
                provider: "deepseek".into(),
                model: "test".into(),
                base_url: "https://test.com".into(),
                max_tokens: 100,
                temperature: 0.1,
                perplexity_key: None,
            },
        )
        .unwrap();
        assert!(matches.is_empty(), "Unrelated question should not match");
    }

    #[test]
    fn test_keyword_match_high_ratio() {
        let analysis = make_test_analysis();
        let question = Question {
            id: "q1".into(),
            text: "semiconductor export controls".into(),
            category: "Tech".into(),
            priority: 8,
        };
        let text = format!(
            "{} {} {}",
            analysis.bluf, analysis.impact, analysis.supply_chain_impact
        );
        let (relevance, _reasoning) = keyword_match(&text, &question);
        assert!(
            relevance >= 5,
            "Keyword match should be high for exact terms: {}",
            relevance
        );
    }

    #[test]
    fn test_keyword_match_no_match() {
        let analysis = make_test_analysis();
        let question = Question {
            id: "q2".into(),
            text: "federal reserve interest rates".into(),
            category: "Macro".into(),
            priority: 6,
        };
        let text = format!(
            "{} {} {}",
            analysis.bluf, analysis.impact, analysis.supply_chain_impact
        );
        let (relevance, _reasoning) = keyword_match(&text, &question);
        assert_eq!(relevance, 0, "Unrelated question should have 0 relevance");
    }

    #[test]
    fn test_question_engine_config_default() {
        let config = QuestionEngineConfig::default();
        assert!(!config.enable_semantic_matching);
    }
}
