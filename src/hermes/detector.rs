//! 变更检测：规则版 + LLM 语义版

use crate::archive::ChronicleEntry;
use crate::domain::theme::ThemeAnalysis;
use crate::config::LlmConfig;

use super::{ChangeDetectionEntry, ChangeSummary, ConflictEntry, SemanticRelation};

/// 检测今日信号与近 7 天 chronicle 的冲突/强化关系（规则版）
///
/// 基于 topic 名称匹配 + 蓝军输出。用于 LLM 版不可用时的 fallback。
/// 冷启动处理：chronicle 为空时返回 all-new。
pub fn detect_changes_rule(
    recent_entries: &[ChronicleEntry],
    analyses: &[ThemeAnalysis],
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
    let conflicts = Vec::new();
    let mut reinforced = Vec::new();
    let mut new_signals = Vec::new();

    for analysis in analyses {
        let title = &analysis.theme_title;

        // 检查 chronicle 中是否有相同 topic
        let prior: Vec<&&str> = recent_topics.iter().filter(|t| t == &title).collect();

        if prior.is_empty() {
            new_signals.push(title.clone());
            continue;
        }

        // 冲突判定：adverse scenario 是分析的一部分（assumptions 的脆弱性标注），
        // 不是新到的挑战信号。Conflict 要求语义对立——今日立场与既有 thesis 方向相反。
        // 重复出现的主题视为 Reinforce。
        reinforced.push(title.clone());
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
    recent_entries: &[ChronicleEntry],
    analyses: &[ThemeAnalysis],
    api_key: &str,
    llm_config: &LlmConfig,
) -> Option<ChangeSummary> {
    // 滑动窗口：取最近 N 条 chronicle 条目参与比对
    const WINDOW: usize = 30;
    let core_entries: Vec<&ChronicleEntry> = recent_entries.iter().take(WINDOW).collect();

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

    let client = crate::llm::create_llm_client()
        .ok()?;

    let raw =
        crate::llm::call_with_retry_raw(&client, api_key, llm_config, system_prompt, &user_prompt)
            .await
            .map_err(|e| {
                log::warn!("LLM Change Detection API 调用失败: {}", e);
            })
            .ok()?;

    // Use shared multi-strategy JSON array parser from llm.rs
    let entries: Vec<ChangeDetectionEntry> = match crate::llm::parse_json_array(&raw) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("LLM Change Detection JSON 解析失败: {}", e);
            return None;
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
