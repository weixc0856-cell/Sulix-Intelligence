//! Verification Agent (🔵 蓝军) — 极致怀疑
//!
//! 对红军的叙事分析做逐条反驳，注入两大核心技能：
//! 1. AI 神话拆解六问
//! 2. 证据等级 L1-L5
//!
//! 角色: 极致怀疑主义者。以驳倒红军为荣。
//! 思维模式: "如果这一切都是错的……"

use std::collections::HashMap;

use anyhow::Result;

use crate::config::LlmConfig;
use crate::llm;

use super::synthesis::{Narrative, SynthesisOutput};

/// 蓝军对一个 vertical 的反驳输出
pub struct VerificationOutput {
    #[allow(dead_code)]
    pub category: String,
    pub rebuttals: Vec<Rebuttal>,
}

/// 单篇反驳
pub struct Rebuttal {
    pub id: String,
    pub title: String,
    pub counter_narrative: String,
    pub evidence_level: String,
    #[allow(dead_code)]
    pub refutation_strength: u8,
    #[allow(dead_code)]
    pub ai_myth_flags: Vec<String>,
}

/// 对红军的输出执行蓝军验证
pub async fn verify(
    synthesis: &[SynthesisOutput],
    base_prompt: &str,
    vertical_overrides: &HashMap<String, String>,
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<Vec<VerificationOutput>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let mut results = Vec::new();
    let batch_size = 5usize;

    for sv in synthesis {
        if sv.narratives.is_empty() {
            continue;
        }

        log::info!(
            "🔵 Verification [{}] — {} 条叙事 (分 {} 批)",
            sv.category,
            sv.narratives.len(),
            sv.narratives.len().div_ceil(batch_size)
        );

        let category_override = vertical_overrides.get(&sv.category).map(|s| s.as_str());
        let system_prompt = build_verification_prompt(base_prompt, category_override, &sv.category);

        let mut rebuttals = Vec::new();
        let total_batches = sv.narratives.len().div_ceil(batch_size);

        for (batch_idx, batch) in sv.narratives.chunks(batch_size).enumerate() {
            if total_batches > 1 {
                log::debug!(
                    "  ↳ 第 {}/{} 批 ({} 条)",
                    batch_idx + 1,
                    total_batches,
                    batch.len()
                );
            }

            let user_prompt = build_verification_user_prompt(&sv.category, batch);

            match llm::call_with_retry(&client, api_key, llm_config, &system_prompt, &user_prompt)
                .await
            {
                Ok(raw_results) => {
                    let by_id: std::collections::HashMap<&str, &llm::AnalyzedArticleRaw> =
                        raw_results
                            .iter()
                            .filter(|r| !r.id.is_empty())
                            .map(|r| (r.id.as_str(), r))
                            .collect();
                    let by_title: std::collections::HashMap<&str, &llm::AnalyzedArticleRaw> =
                        raw_results.iter().map(|r| (r.title.as_str(), r)).collect();

                    for n in batch {
                        let matched = by_id
                            .get(n.id.as_str())
                            .copied()
                            .or_else(|| by_title.get(n.title.as_str()).copied());

                        if let Some(r) = matched {
                            rebuttals.push(Rebuttal {
                                id: n.id.clone(),
                                title: r.title.clone(),
                                counter_narrative: r.judgment.clone(),
                                evidence_level: r.confidence.clone(),
                                refutation_strength: r.importance.clamp(1, 10),
                                ai_myth_flags: Vec::new(),
                            });
                        } else {
                            rebuttals.push(Rebuttal {
                                id: n.id.clone(),
                                title: n.title.clone(),
                                counter_narrative: String::new(),
                                evidence_level: "未匹配".into(),
                                refutation_strength: 0,
                                ai_myth_flags: vec!["蓝军未返回此条".into()],
                            });
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "⚠️ Verification [{}] 第{}批失败: {}，该批降级处理",
                        sv.category,
                        batch_idx + 1,
                        e
                    );
                    for n in batch {
                        rebuttals.push(Rebuttal {
                            id: n.id.clone(),
                            title: n.title.clone(),
                            counter_narrative: String::new(),
                            evidence_level: "未分析".into(),
                            refutation_strength: 0,
                            ai_myth_flags: vec!["蓝军未执行".into()],
                        });
                    }
                }
            }

            if total_batches > 1 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }

        log::info!(
            "✅ Verification [{}]: {} 条反驳",
            sv.category,
            rebuttals.len()
        );
        results.push(VerificationOutput {
            category: sv.category.clone(),
            rebuttals,
        });
    }

    Ok(results)
}

/// 构建 Verification system prompt
///
/// = base prompt + 蓝军角色注入 + 证据等级 + 六问 + override + JSON 约束
fn build_verification_prompt(base: &str, override_text: Option<&str>, category: &str) -> String {
    let mut prompt = base.to_string();

    // 注入蓝军角色定义（执行风险视角，非技术反驳）
    prompt.push_str(
        "\n\n【你的角色：蓝军 🔵 — 风险审计员】\n\
        你的任务是评估红军提出的商业机会有什么隐藏成本。\
        你不是来反驳技术细节，而是来审问商业现实。\n\n\
        核心追问：\n\
        1. 这个商业机会的执行障碍在哪？（客户获取？技术债？合规？）\n\
        2. 如果这个判断是错的，最可能的原因是什么？\n\
        3. 个人创业者做这件事的真实成本（不只有代码成本）？\n\
        4. 证据等级：这是基于确凿事实(L1/L2)还是推测(L4/L5)？\n\n\
        思维模式：\"如果这是一门生意，哪里会亏钱？\"\n\
        💡 注意：不谈benchmark和技术细节，只谈商业风险。",
    );

    if let Some(ov) = override_text {
        prompt.push_str("\n\n");
        prompt.push_str(ov);
    }

    prompt.push_str(&format!("\n\n当前领域：{}", category));

    // JSON 格式约束
    prompt.push_str(
        "\n\n你必须以 JSON 格式输出。格式如下（严格遵循）：\n\
        {\n  \
        \"articles\": [\n    \
        {\n      \
        \"id\": \"文章的 ID（从输入获取，严格保持原样）\",\n      \
        \"title\": \"红军分析的原文标题\",\n      \
        \"importance\": 7,\n      \
        \"relevance\": \"高/中/低\",\n      \
        \"time_horizon\": \"短期/中期/长期\",\n      \
        \"action\": \"立即行动/研究/观察/忽略\",\n      \
        \"confidence\": \"L1/L2/L3/L4/L5\",\n      \
        \"judgment\": \"反驳分析（2-4句话，指出漏洞和过度推演）\"\n    \
        }\n  \
        ]\n\
        }\n\n\
        注意事项：\n\
        1. importance 表示反驳力度（1-10），越高反驳越有力\n\
        2. confidence 请使用证据等级 L1/L2/L3/L4/L5\n\
        3. judgment 必须包含反驳逻辑和使用了哪个拆解技能\n\
        4. 为每条红军叙事都生成一条反驳结果，数量严格对应\n\
        5. id 字段必须从输入中获取并严格保持原样\n\
        6. 输出纯 JSON，不要在前后加任何说明文字",
    );

    prompt
}

/// 构建 Verification user prompt
///
/// 将红军的叙事分析发给蓝军做反驳
fn build_verification_user_prompt(category: &str, narratives: &[Narrative]) -> String {
    let mut prompt = format!(
        "请对以下 {} 领域的 {} 条红军叙事分析进行反驳，输出 JSON：\n\n",
        category,
        narratives.len(),
    );

    for (i, n) in narratives.iter().enumerate() {
        prompt.push_str(&format!(
            "--- 叙事 {} ---\nID: {}\n标题: {}\n叙事: {}\n推演逻辑: {}\n信号强度: {}/10\n\n",
            i + 1,
            n.id,
            n.title,
            if n.narrative.is_empty() {
                "(分析失败，无红军叙事)"
            } else {
                &n.narrative
            },
            n.reasoning,
            n.signal_strength,
        ));
    }

    prompt
}
