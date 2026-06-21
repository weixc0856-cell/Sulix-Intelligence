//! Editor Agent (🎯 幕僚长) — 信念匹配引擎
//!
//! 在红蓝对抗之前运行，职责：
//! 1. 将每日信息匹配到信念系统（belief system）
//! 2. 判断每条信息支持还是挑战哪个信念
//! 3. 输出信念更新（belief_id + evidence_type + article）
//!
//! 设计原则：
//! - 只收脱水数据（index + title + category + 100字摘要）
//! - 返回 belief_id 索引，Rust 端按 belief_id 分组
//! - 单次 LLM 调用，不逐个分析

use anyhow::Result;
use serde::Deserialize;

use crate::config::{BeliefStatement, LlmConfig};
use crate::fetcher::Article;
use crate::llm;

/// Editor 匹配结果：一篇文章 + 它对应的信念
#[derive(Debug)]
pub struct BeliefMatch {
    pub article: Article,
    pub routed_category: String,
    #[allow(dead_code)]
    pub belief_id: String,
    #[allow(dead_code)]
    pub evidence_type: String,
    #[allow(dead_code)]
    pub evidence_summary: String,
}

/// Editor API 返回体
#[derive(Debug, Deserialize)]
struct EditorResponse {
    matches: Vec<EditorItem>,
}

#[derive(Debug, Deserialize)]
struct EditorItem {
    index: usize,
    /// 匹配的 belief_id
    belief_id: String,
    /// "support" 或 "challenge"
    evidence_type: String,
    category: String,
    /// 一句话证据摘要（用高密度黑话）
    evidence_summary: String,
}

/// 信念匹配：将 articles 匹配到 belief_statements
///
/// world_state: 昨日简报摘要
pub async fn match_to_beliefs(
    articles: &[Article],
    world_state: &str,
    beliefs: &[BeliefStatement],
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<Vec<BeliefMatch>> {
    if articles.is_empty() || beliefs.is_empty() {
        return Ok(Vec::new());
    }

    // 1. 构建脱水输入
    let mut input = String::new();
    for (idx, art) in articles.iter().enumerate() {
        let desc = art
            .summary
            .as_deref()
            .or(art.content.as_deref())
            .unwrap_or("(无摘要)");
        let end = desc.floor_char_boundary(100);
        let truncated: &str = if desc.len() > end { &desc[..end] } else { desc };
        input.push_str(&format!(
            "[{}] {} (分类: {})\n摘要: {}\n---\n",
            idx, art.title, art.category, truncated
        ));
    }

    let total = articles.len();
    log::info!("🎯 Editor Agent: {} 篇待匹配到信念系统", total);

    // 2. 构建信念列表文本
    let beliefs_text: String = beliefs
        .iter()
        .map(|b| {
            format!(
                "  [{}] {} (置信度: {}/10)",
                b.id, b.statement, b.base_confidence
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // 3. 系统 Prompt（信念匹配核心）
    let system_prompt = format!(
        r#"你是一个个人创业者的 Chief of Staff（幕僚长）。你的职责不是筛选新闻，而是判断今天的每条信息**是否为某个核心信念添加了新的证据**。

## 当前信念系统

{}

## 当前世界状态（昨日简报）

{}

## 你的任务

对每条信息，判断它属于以下哪一类：

1. **支持信念** = 这条信息为某个信念提供了新的支持证据
2. **挑战信念** = 这条信息为某个信念提供了反例或挑战
3. **无关** = 这条信息与所有信念无关（跳过，不输出）

## 黄金标尺

1. **范式穿透力**：这是常规迭代还是砸碎游戏规则？
2. **多维共振/背离**：多个维度指向同一方向还是存在背离？
3. **房间里的大象**：谁指出了致命的执行风险？

## 输出规则

只输出 evidence_type 为 support 或 challenge 的条目。每篇文章最多匹配一个信念。

JSON 格式：
{{"matches": [
  {{"index": 序号, "belief_id": "信念ID", "evidence_type": "support/challenge", "category": "路由分类(AI/技术主线/创业/A股/芯片/政策)", "evidence_summary": "一句话证据（用高密度黑话）"}}
]}}

约束：
- 如果某信息与所有信念无关 → 跳过（不输出）
- belief_id 必须完全匹配信念系统中的 ID
- 最多输出 10 条"#,
        beliefs_text, world_state
    );

    // 4. 调用 LLM
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let raw = llm::call_raw(&client, api_key, llm_config, &system_prompt, &input).await?;

    // 5. 解析响应
    let parsed = parse_editor_response(&raw)?;

    // 6. 按 index 匹配原文章
    let mut matches = Vec::new();
    for item in parsed.matches {
        if let Some(article) = articles.get(item.index) {
            log::info!(
                "  🎯 [{}] {} → belief:{} ({})",
                item.index,
                article.title,
                item.belief_id,
                item.evidence_type
            );
            matches.push(BeliefMatch {
                article: article.clone(),
                routed_category: item.category,
                belief_id: item.belief_id,
                evidence_type: item.evidence_type,
                evidence_summary: item.evidence_summary,
            });
        } else {
            log::warn!("⚠️ Editor 返回了越界 index {}，跳过", item.index);
        }
    }

    log::info!("🎯 Editor Agent: {}/{} 篇匹配到信念", matches.len(), total);
    Ok(matches)
}

fn parse_editor_response(raw: &str) -> Result<EditorResponse> {
    if let Ok(parsed) = serde_json::from_str::<EditorResponse>(raw) {
        return Ok(parsed);
    }
    if let Some(block) = raw.split("```json\n").nth(1) {
        if let Some(json) = block.split("```").next() {
            if let Ok(parsed) = serde_json::from_str::<EditorResponse>(json.trim()) {
                return Ok(parsed);
            }
        }
    }
    if let Some(start) = raw.find('{') {
        if let Some(end) = raw.rfind('}') {
            if end > start {
                let sliced = &raw[start..=end];
                if let Ok(parsed) = serde_json::from_str::<EditorResponse>(sliced) {
                    return Ok(parsed);
                }
            }
        }
    }
    Err(anyhow::anyhow!(
        "Editor: 无法解析 LLM 响应: {}...",
        &raw[..raw.len().min(200)]
    ))
}
