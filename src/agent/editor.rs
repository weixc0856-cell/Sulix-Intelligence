//! Editor Agent (🎯 幕僚长) — 问题匹配引擎
//!
//! 判断每条信息回答了你当前正在思考的哪个决策问题。
//! 允许一条信息同时回答多个问题，每条匹配独立输出。

use anyhow::Result;
use serde::Deserialize;

use crate::config::{BeliefStatement, LlmConfig};
use crate::fetcher::Article;
use crate::llm;

/// 匹配结果
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

#[derive(Debug, Deserialize)]
struct EditorResponse {
    matches: Vec<EditorItem>,
}

#[derive(Debug, Deserialize)]
struct EditorItem {
    index: usize,
    question_id: String,
    impact: String,
    strength: String,
    category: String,
    evidence: String,
}

/// 问题匹配
pub async fn match_to_beliefs(
    articles: &[Article],
    _world_state: &str,
    beliefs: &[BeliefStatement],
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<Vec<BeliefMatch>> {
    if articles.is_empty() || beliefs.is_empty() {
        return Ok(Vec::new());
    }

    // 脱水输入
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
    log::info!("🎯 Editor Agent: {} 篇待匹配", total);

    let questions_text: String = beliefs
        .iter()
        .map(|b| format!("  [{}] {}", b.id, b.statement))
        .collect::<Vec<_>>()
        .join("\n");

    let system_prompt = format!(
        r#"你是一个个人创业者的幕僚长。对每条信息，判断它是否影响你正在思考的决策问题。

## 你的问题
{}

## 判断规则
- 如果信息改变了你对某个问题的看法 → 匹配
- 如果信息为某个问题提供了新的证据 → 匹配
- 无关才跳过

示例：
- "GLM-5.2发布" → d1, d2
- "Anthropic下线" → d2, d4
- "Fable出口限制" → d2
- "DOS游戏" → 跳过

## 输出 JSON
{{"matches": [
  {{"index": 序号, "question_id": "问题ID", "impact": "support/challenge", "strength": "high/medium", "category": "路由分类", "evidence": "一句话证据"}}
]}}

最多输出 20 条"#,
        questions_text
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let raw = llm::call_raw(&client, api_key, llm_config, &system_prompt, &input).await?;
    log::info!("📥 Editor LLM 响应: {}...", &raw[..raw.len().min(300)]);

    let parsed = match parse_editor_response(&raw) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("⚠️ Editor 解析失败: {}，回退到空匹配", e);
            return Ok(Vec::new());
        }
    };

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
            matches.push(BeliefMatch {
                article: article.clone(),
                routed_category: item.category,
                belief_id: item.question_id,
                evidence_type: if item.impact == "challenge" {
                    "challenge".into()
                } else {
                    format!("{}_{}", item.impact, item.strength)
                },
                evidence_summary: item.evidence,
            });
        }
    }

    log::info!("🎯 Editor Agent: {} 条匹配", matches.len());
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
    Err(anyhow::anyhow!("无法解析: {}", &raw[..raw.len().min(200)]))
}
