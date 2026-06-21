//! Editor Agent (🎯 幕僚长) — 问题匹配引擎
//!
//! 判断每条信息回答了你当前正在思考的哪个决策问题。
//! 允许一条信息同时回答多个问题，每条匹配独立输出。

use anyhow::Result;
use serde::Deserialize;

use crate::config::{BeliefStatement, LlmConfig};
use crate::fetcher::Article;
use crate::llm;

/// 匹配结果：一篇文章可以产生多条匹配（回答多个问题）
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
    question_id: String,
    /// support 或 challenge
    impact: String,
    /// high 或 medium
    strength: String,
    category: String,
    evidence: String,
}

/// 问题匹配：将 articles 匹配到当前正在思考的问题
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
    log::info!("🎯 Editor Agent: {} 篇待匹配到问题", total);

    // 2. 构建问题列表
    let questions_text: String = beliefs
        .iter()
        .map(|b| format!("  [{}] {}", b.id, b.statement))
        .collect::<Vec<_>>()
        .join("\n");

    // 3. 系统 Prompt（问题匹配核心）
    let system_prompt = format!(
        r#"你是一个个人创业者的 Chief of Staff（幕僚长）。你的职责不是筛选新闻，而是判断每天的每条信息**回答了你当前正在思考的哪个决策问题**。

## 你当前正在思考的问题

{}

## 当前世界状态（昨日简报）

{}

## 你的任务

对每条信息，判断它回答（或影响了）哪个问题。必须严格：
- 只选**直接相关**的信息，弱关联不匹配
- 一条信息可以同时影响多个问题（最多2个）
- 无关信息跳过，不输出

## 影响类型

- **support** = 支持/强化对该问题的现有看法
- **challenge** = 挑战/动摇了对该问题的看法

## 强度

- **high** = 直接影响，强信号
- **medium** = 间接影响，弱信号

## 输出

JSON 格式：
{{"matches": [
  {{"index": 序号, "question_id": "问题ID", "impact": "support/challenge", "strength": "high/medium", "category": "路由分类(AI/技术主线/创业/A股/芯片/政策)", "evidence": "一句话证据（30字内，用高密度黑话）"}}
]}}

约束：
- 同一 index 可以出现多次（同时回答多个问题）
- question_id 必须完全匹配问题ID
- 最多输出 15 条"#,
        questions_text, world_state
    );

    // 4. 调用 LLM
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let raw = llm::call_raw(&client, api_key, llm_config, &system_prompt, &input).await?;

    // 5. 解析响应
    let parsed = parse_editor_response(&raw)?;

    // 6. 按 index 匹配原文章，允许一篇文章多条匹配
    let mut matches = Vec::new();
    for item in parsed.matches {
        if let Some(article) = articles.get(item.index) {
            log::info!(
                "  🎯 [{}] {} → {} ({}/{})",
                item.index,
                article.title,
                item.question_id,
                item.impact,
                item.strength
            );
            let evidence_type = if item.impact == "challenge" {
                "challenge".into()
            } else {
                format!("{}_{}", item.impact, item.strength)
            };
            matches.push(BeliefMatch {
                article: article.clone(),
                routed_category: item.category,
                belief_id: item.question_id,
                evidence_type,
                evidence_summary: item.evidence,
            });
        } else {
            log::warn!("⚠️ Editor 返回了越界 index {}，跳过", item.index);
        }
    }

    log::info!(
        "🎯 Editor Agent: {} 条匹配（{}/{} 篇）",
        matches.len(),
        total,
        total
    );
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
