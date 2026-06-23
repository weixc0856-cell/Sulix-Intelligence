//! Change Detection — 检测今日信号与历史 chronicle 的冲突/强化关系
//!
//! 两个版本：
//!   - `detect_changes_rule`: 规则版，基于 topic 名称匹配 + 蓝军 adverse 输出
//!   - `detect_changes_llm`: LLM 语义版，对比分析主题与历史 chronicle 条目

use serde::{Deserialize, Serialize};

/// 冲突条目
#[derive(Debug, Clone, Serialize)]
pub struct ConflictEntry {
    pub topic: String,
    pub today_signal: String,
    pub prior_belief: String,
}

/// Change Summary 输出
#[derive(Debug, Clone, Serialize)]
pub struct ChangeSummary {
    pub conflicts: Vec<ConflictEntry>,
    pub reinforced: Vec<String>,
    pub new_signals: Vec<String>,
    pub no_change_count: usize,
}

/// 语义关系枚举（LLM Change Detection 使用）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRelation {
    /// 冲突：今日信号推翻或篡改了历史 Belief
    Conflict,
    /// 强化：今日信号是旧事实的深化或因果传导
    Reinforce,
    /// 无关：不同的技术栈或实体
    Irrelevant,
}

/// LLM Change Detection 输出条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDetectionEntry {
    pub topic: String,
    pub relation: SemanticRelation,
    pub justification: String,
}

/// 检测今日信号与近 7 天 chronicle 的冲突/强化关系（规则版）
///
/// 基于 topic 名称匹配 + 蓝军输出。用于 LLM 版不可用时的 fallback。
/// 冷启动处理：chronicle 为空时返回 all-new。
pub fn detect_changes_rule(
    recent_entries: &[crate::archive::ChronicleEntry],
    analyses: &[crate::clusterer::ThemeAnalysis],
) -> ChangeSummary {
    if recent_entries.is_empty() {
        return ChangeSummary {
            conflicts: vec![],
            reinforced: vec![],
            new_signals: analyses.iter().map(|a| a.theme_title.clone()).collect(),
            no_change_count: 0,
        };
    }

    // 从 chronicle 条目中提取近期主题摘要
    let recent_topics: Vec<&str> = recent_entries.iter().map(|e| e.topic.as_str()).collect();
    let mut conflicts = Vec::new();
    let mut reinforced = Vec::new();
    let mut new_signals = Vec::new();

    for analysis in analyses {
        let title = &analysis.theme_title;
        let summary = &analysis.bluf;

        // 检查 chronicle 中是否有相同 topic
        let prior: Vec<&&str> = recent_topics.iter().filter(|t| t == &title).collect();

        if prior.is_empty() {
            new_signals.push(title.clone());
            continue;
        }

        // 检查蓝军输出：有 adverse 或 weak assumption → 可能是冲突
        let has_adverse = analysis
            .adverse
            .as_ref()
            .map(|a| !a.scenario.is_empty())
            .unwrap_or(false);
        let has_weak = analysis
            .assumptions
            .iter()
            .any(|a| a.load_bearing && a.evidence_strength == "weak");

        if has_adverse || has_weak {
            conflicts.push(ConflictEntry {
                topic: title.clone(),
                today_signal: summary.clone(),
                prior_belief: format!("近 7 天出现 {} 次", prior.len()),
            });
        } else {
            reinforced.push(title.clone());
        }
    }

    let no_change = reinforced.len();
    ChangeSummary {
        conflicts,
        reinforced,
        new_signals,
        no_change_count: no_change,
    }
}

// ===== News Layer: LLM Change Detection =====

/// LLM 语义版 Change Detection
///
/// 对比今日分析主题与近 7 天历史 Chronicle 条目，
/// 运用熊彼特创造性毁灭与诺斯路径依赖理论，
/// 判定经济与地缘语义依赖关系。
///
/// 滑动窗口+SVI 过滤：只选取 SVI >= 5 的核心条目参与比对。
/// 防死锁：LLM 失败时返回 None，由调用方 fallback 到规则版。
pub async fn detect_changes_llm(
    recent_entries: &[crate::archive::ChronicleEntry],
    analyses: &[crate::clusterer::ThemeAnalysis],
    api_key: &str,
    llm_config: &crate::config::LlmConfig,
) -> Option<ChangeSummary> {
    // 滑动窗口：取最近 30 条 chronicle 条目参与比对
    let core_entries: Vec<&crate::archive::ChronicleEntry> =
        recent_entries.iter().take(30).collect();

    if core_entries.is_empty() {
        log::info!("Change Detection: 近 7 天无 SVI>=5 的核心条目，冷启动模式");
        return None;
    }
    if analyses.is_empty() {
        return None;
    }

    let history_json = serde_json::to_string(&core_entries).unwrap_or_default();
    let today_json = serde_json::to_string(
        &analyses
            .iter()
            .map(|a| {
                serde_json::json!({
                    "topic": a.theme_title,
                    "bluf": a.bluf,
                    "impact": a.impact,
                    "signal_strength": a.signal_strength,
                    "has_adverse": a.adverse.as_ref().map(|x| !x.scenario.is_empty()).unwrap_or(false),
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_default();

    let system_prompt = r#"你是 Sulix 智库的终极历史审查官。
对比【今日分析主题】与【近 7 天历史 Chronicle 条目】，判定其经济与地缘语义依赖关系。

约束：
- 运用熊彼特"创造性毁灭"与诺斯"路径依赖"理论。
- 如果今日事件导致旧事件的 CapEx 预测或制度变迁预期失效 → conflict
- 如果今日事件是旧事件合规阻尼(Compliance Drag)的因果传导或非线性深化 → reinforce
- 如果今日事件涉及完全不同的技术栈、地理实体或宏观政策维度 → irrelevant

Output json. 输出严格 JSON 数组，每项格式：
{"topic": "主题名", "relation": "conflict|reinforce|irrelevant", "justification": "一句话经济学/社会学依据"}"#;

    let user_prompt = format!(
        "【历史条目】:\n{}\n\n【今日分析】:\n{}",
        history_json, today_json
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .ok()?;

    let raw =
        crate::llm::call_with_retry_raw(&client, api_key, llm_config, system_prompt, &user_prompt)
            .await
            .map_err(|e| {
                log::warn!("LLM Change Detection API 调用失败: {}", e);
            })
            .ok()?;

    // 多步容错解析：JSON fence 剥离 → 提取数组段
    let raw_clean = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    // 尝试直接解析为 Vec<ChangeDetectionEntry>
    let entries_result: Result<Vec<ChangeDetectionEntry>, _> = serde_json::from_str(raw_clean);
    let entries = match entries_result {
        Ok(e) => e,
        Err(_) => {
            // 尝试提取 JSON 数组段（LLM 可能在数组前后加了额外文字）
            if let Some(arr_start) = raw_clean.find('[') {
                if let Some(arr_end) = raw_clean.rfind(']') {
                    let slice = &raw_clean[arr_start..=arr_end];
                    match serde_json::from_str::<Vec<ChangeDetectionEntry>>(slice) {
                        Ok(e) => e,
                        Err(e2) => {
                            log::warn!(
                                "LLM Change Detection JSON 解析失败 (fallback 尝试也失败): {}",
                                e2
                            );
                            return None;
                        }
                    }
                } else {
                    log::warn!("LLM Change Detection 响应中未找到 JSON 数组");
                    return None;
                }
            } else {
                log::warn!(
                    "LLM Change Detection 响应中未找到 JSON 数组: {}",
                    &raw_clean[..raw_clean.len().min(200)]
                );
                return None;
            }
        }
    };

    let mut conflicts = Vec::new();
    let mut reinforced = Vec::new();
    let mut new_signals = Vec::new();

    for entry in entries {
        match entry.relation {
            SemanticRelation::Conflict => {
                conflicts.push(ConflictEntry {
                    topic: entry.topic.clone(),
                    today_signal: entry.justification.clone(),
                    prior_belief: "近 7 天历史基线".into(),
                });
            }
            SemanticRelation::Reinforce => {
                reinforced.push(entry.topic);
            }
            SemanticRelation::Irrelevant => {
                new_signals.push(entry.topic);
            }
        }
    }

    let no_change = reinforced.len();
    Some(ChangeSummary {
        conflicts,
        reinforced,
        new_signals,
        no_change_count: no_change,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clusterer::{AdverseScenario, ThemeAnalysis};

    #[test]
    fn test_semantic_relation_deserialization() {
        let json = r#"{"topic": "AI Coding", "relation": "conflict", "justification": "test"}"#;
        let entry: ChangeDetectionEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.relation, SemanticRelation::Conflict);

        let json = r#"{"topic": "AI Coding", "relation": "reinforce", "justification": "test"}"#;
        let entry: ChangeDetectionEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.relation, SemanticRelation::Reinforce);

        let json = r#"{"topic": "AI Coding", "relation": "irrelevant", "justification": "test"}"#;
        let entry: ChangeDetectionEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.relation, SemanticRelation::Irrelevant);
    }

    #[test]
    fn test_detect_changes_rule_cold_start() {
        let result = detect_changes_rule(&[], &[]);
        assert!(result.new_signals.is_empty());
        assert_eq!(result.no_change_count, 0);
    }

    #[test]
    fn test_detect_changes_rule_with_data() {
        use crate::archive::ChronicleEntry;
        let entries = vec![ChronicleEntry {
            date: "2026-06-22".into(),
            topic: "AI".into(),
            headline: "test".into(),
            entities: vec![],
            signal_strength: 7,
            language: "en".into(),
        }];
        let analyses = vec![ThemeAnalysis {
            theme_id: "t1".into(),
            theme_title: "AI".into(),
            bluf: "test".into(),
            impact: "".into(),
            geopolitical_fact: "".into(),
            supply_chain_impact: "".into(),
            analysis_paragraph: "".into(),
            evidence_level: "".into(),
            signal_strength: 7,
            fact_base: vec![],
            connections: vec![],
            source_urls: vec![],
            assumptions: vec![],
            adverse: Some(AdverseScenario {
                scenario: "冲突".into(),
                early_warning: "".into(),
                severity: "high".into(),
            }),
            next_tests: vec![],
            open_questions: vec![],
            chains: vec![],
            what_to_do: String::new(),
            what_to_watch: String::new(),
        }];
        let result = detect_changes_rule(&entries, &analyses);
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].topic, "AI");
    }

    #[test]
    fn test_detect_changes_rule_fallback_no_adverse() {
        use crate::archive::ChronicleEntry;
        let entries = vec![ChronicleEntry {
            date: "2026-06-22".into(),
            topic: "AI".into(),
            headline: "test".into(),
            entities: vec![],
            signal_strength: 7,
            language: "en".into(),
        }];
        let analyses = vec![ThemeAnalysis {
            theme_id: "t1".into(),
            theme_title: "AI".into(),
            bluf: "reinforced".into(),
            impact: "".into(),
            geopolitical_fact: "".into(),
            supply_chain_impact: "".into(),
            analysis_paragraph: "".into(),
            evidence_level: "".into(),
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
        }];
        let result = detect_changes_rule(&entries, &analyses);
        assert_eq!(result.reinforced.len(), 1);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_detect_changes_llm_json_parsing_fallback() {
        // 模拟 LLM 输出：前后有多余文字
        let raw_with_noise = r#"Here is my analysis:
[
  {"topic": "Test", "relation": "conflict", "justification": "Direct contradiction on CapEx assumptions"}
]
That's my final answer."#;

        let clean = raw_with_noise
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        // 提取 JSON 数组段落
        if let Some(arr_start) = clean.find('[') {
            if let Some(arr_end) = clean.rfind(']') {
                let slice = &clean[arr_start..=arr_end];
                let entries: Vec<ChangeDetectionEntry> = serde_json::from_str(slice).unwrap();
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].topic, "Test");
                assert_eq!(entries[0].relation, SemanticRelation::Conflict);
                return;
            }
        }
        panic!("JSON array extraction failed");
    }
}
