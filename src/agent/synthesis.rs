//! Synthesis Agent (🔴 红军) — 乐观叙事
//!
//! 对每组文章做轻量扫描后的深度分析，输出乐观串联的叙事。
//! 复用 llm::call_with_retry 做 API 调用（指数退避 + 4xx 不重试）。
//!
//! 角色: 乐观主义者。寻找蛛丝马迹，串联成宏大叙事。
//! 思维模式: "如果这一切都是真的……"

use anyhow::Result;
use std::collections::HashMap;

use crate::config::LlmConfig;
use crate::fetcher::Article;
use crate::llm;

type OverrideMap = HashMap<String, String>;

/// 红军对一个 vertical 的分析输出
pub struct SynthesisOutput {
    pub category: String,
    pub narratives: Vec<Narrative>,
}

/// 单篇文章的叙事分析（包含完整 5 维判断框架）
pub struct Narrative {
    pub id: String,
    pub title: String,
    pub narrative: String,
    pub reasoning: String,
    pub signal_strength: u8,
    pub relevance: String,
    pub time_horizon: String,
    pub action: String,
    pub confidence: String,
}

/// 对分组后的文章执行红军（Synthesis）分析
pub async fn synthesize(
    grouped: &HashMap<String, Vec<Article>>,
    base_prompt: &str,
    vertical_overrides: &OverrideMap,
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<Vec<SynthesisOutput>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let mut results = Vec::new();
    let batch_size = 8usize;

    for (category, articles) in grouped {
        if articles.is_empty() {
            continue;
        }

        log::info!(
            "🔴 Synthesis [{}] — {} 篇 (分 {} 批)",
            category,
            articles.len(),
            articles.len().div_ceil(batch_size)
        );

        let category_override = vertical_overrides.get(category).map(|s| s.as_str());
        let system_prompt = build_synthesis_prompt(base_prompt, category_override, category);
        let mut narratives = Vec::new();

        for (batch_idx, batch) in articles.chunks(batch_size).enumerate() {
            if articles.len() > batch_size {
                log::debug!(
                    "  ↳ 第 {}/{} 批 ({} 篇)",
                    batch_idx + 1,
                    articles.len().div_ceil(batch_size),
                    batch.len()
                );
            }

            let user_prompt = build_synthesis_user_prompt(category, batch_idx + 1, batch);

            match llm::call_with_retry(&client, api_key, llm_config, &system_prompt, &user_prompt)
                .await
            {
                Ok(raw_results) => {
                    // 建立 batch 内 id→Article 的索引（用于 LLM 未回传 id 时的 fallback）
                    let id_map: HashMap<&str, &Article> =
                        batch.iter().map(|a| (a.id.as_str(), a)).collect();
                    let title_map: HashMap<&str, &Article> =
                        batch.iter().map(|a| (a.title.as_str(), a)).collect();

                    for entry in raw_results {
                        // 按 id 匹配原文章，fallback 到 title
                        let article = (!entry.id.is_empty())
                            .then(|| id_map.get(entry.id.as_str()).copied())
                            .flatten()
                            .or_else(|| title_map.get(entry.title.as_str()).copied());

                        let article_id = article.map(|a| a.id.clone()).unwrap_or_default();

                        narratives.push(Narrative {
                            id: article_id,
                            title: entry.title,
                            narrative: entry.judgment,
                            reasoning: format!(
                                "重要性: {}/10 | 相关性: {} | 可行动性: {}",
                                entry.importance, entry.relevance, entry.action
                            ),
                            signal_strength: entry.importance.clamp(1, 10),
                            relevance: if entry.relevance.is_empty() {
                                "未分析".into()
                            } else {
                                entry.relevance
                            },
                            time_horizon: if entry.time_horizon.is_empty() {
                                "未分析".into()
                            } else {
                                entry.time_horizon
                            },
                            action: if entry.action.is_empty() {
                                "观察".into()
                            } else {
                                entry.action
                            },
                            confidence: if entry.confidence.is_empty() {
                                "低".into()
                            } else {
                                entry.confidence
                            },
                        });
                    }
                }
                Err(e) => {
                    log::warn!(
                        "⚠️ Synthesis [{}] 第{}批失败 ({})，降级为原始条目",
                        category,
                        batch_idx + 1,
                        e
                    );
                    for a in batch {
                        narratives.push(Narrative {
                            id: a.id.clone(),
                            title: a.title.clone(),
                            narrative: String::new(),
                            reasoning: "LLM 分析失败，使用原始条目".into(),
                            signal_strength: 5,
                            relevance: "未分析".into(),
                            time_horizon: "未分析".into(),
                            action: "观察".into(),
                            confidence: "低".into(),
                        });
                    }
                }
            }

            if articles.len() > batch_size {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
        }

        log::info!("✅ Synthesis [{}]: {} 条叙事", category, narratives.len());
        results.push(SynthesisOutput {
            category: category.clone(),
            narratives,
        });
    }

    Ok(results)
}

/// 构建 Synthesis system prompt
///
/// = base prompt + 红军角色注入 + vertical override + JSON 格式约束
fn build_synthesis_prompt(base: &str, override_text: Option<&str>, category: &str) -> String {
    let mut prompt = base.to_string();

    // 注入红军角色定义（硬编码——确保护城河不被配置覆盖）
    prompt.push_str(
        "\n\n【你的角色：红军 🔴】\n\
        你是一个乐观主义者。你的任务是从创业者视角寻找蛛丝马迹，\
        把零散的信息串联成有洞见的叙事。\n\n\
        核心技能：\n\
        1. 跨源关联：不同来源的报道之间有什么隐藏联系？\n\
        2. 趋势推演：这些信息组合在一起预示什么趋势？\n\
        3. 生态位分析：这条信息对个人创业者的哪个生态位有机会？\n\n\
        思维模式：\"如果这一切都是真的……\"",
    );

    if let Some(ov) = override_text {
        prompt.push_str("\n\n");
        prompt.push_str(ov);
    }

    prompt.push_str(&format!("\n\n当前领域：{}", category));

    // 复用现有 JSON 格式约束（从 llm.rs 拷贝关键部分）
    prompt.push_str(
        "\n\n你必须以 JSON 格式输出。格式如下（严格遵循）：\n\
        {\n  \
        \"articles\": [\n    \
        {\n      \
        \"id\": \"文章的 ID（从输入获取，严格保持原样）\",\n      \
        \"title\": \"文章标题\",\n      \
        \"importance\": 7,\n      \
        \"relevance\": \"高/中/低\",\n      \
        \"time_horizon\": \"短期/中期/长期\",\n      \
        \"action\": \"立即行动/研究/观察/忽略\",\n      \
        \"confidence\": \"高/中/低\",\n      \
        \"judgment\": \"乐观叙事分析（2-4句话，跨源串联）\"\n    \
        }\n  \
        ]\n\
        }\n\n\
        注意事项：\n\
        1. importance 必须是 1-10 的整数\n\
        2. relevance、time_horizon、action、confidence 必须使用指定的枚举值\n\
        3. judgment 必须包含从创业者视角的乐观叙事解读\n\
        4. 为每篇输入文章都生成一条分析结果，数量严格对应\n\
        5. id 字段必须从输入原文中获取并严格保持原样\n\
        6. 输出纯 JSON，不要在前后加任何说明文字",
    );

    prompt
}

/// 构建 Synthesis user prompt
fn build_synthesis_user_prompt(category: &str, batch_idx: usize, articles: &[Article]) -> String {
    let mut prompt = format!(
        "请从红军视角分析以下 {} 领域的 {} 条新闻（第 {} 批），输出 JSON：\n\n",
        category,
        articles.len(),
        batch_idx,
    );

    for (i, article) in articles.iter().enumerate() {
        prompt.push_str(&format!(
            "--- 文章 {} ---\nID: {}\n标题: {}\n链接: {}\n来源: {}\n",
            i + 1,
            article.id,
            article.title,
            article.url,
            article.source,
        ));

        let body = article
            .content
            .as_deref()
            .or(article.summary.as_deref())
            .unwrap_or("(无全文)");

        let truncated = if body.len() > 3000 {
            let end = body.floor_char_boundary(3000);
            format!("{}...", &body[..end])
        } else {
            body.to_string()
        };

        prompt.push_str(&format!("正文: {}\n\n", truncated));
    }

    prompt
}
