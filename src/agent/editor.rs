//! Editor Agent (🎯 幕僚长) — 认知压缩与 Lens 路由
//!
//! 在红蓝对抗之前运行，职责：
//! 1. 从 Scan Agent 过滤后的文章中选出 3-5 条最重要的
//! 2. 为每条选中的文章路由到正确的分析 Lens（分类）
//! 3. 结合"世界状态"（昨日简报摘要）判断增量价值
//!
//! 设计原则：
//! - 只收脱水数据（index + title + category + 100字摘要）
//! - 返回 index 列表，Rust 端按 index 匹配原文
//! - 单次 LLM 调用，不逐个分析
//! - 调用 llm::call_raw 获取原始文本后再 parse

use anyhow::Result;
use serde::Deserialize;

use crate::config::LlmConfig;
use crate::fetcher::Article;
use crate::llm;

/// Editor 选中的单篇文章
#[derive(Debug)]
pub struct EditorSelection {
    pub article: Article,
    pub routed_category: String,
    /// 挑战的认知编号（999=不挑战任何认知）
    #[allow(dead_code)]
    pub thesis_index: usize,
    #[allow(dead_code)]
    pub reason: String,
}

/// Editor API 返回体
#[derive(Debug, Deserialize)]
struct EditorResponse {
    selections: Vec<EditorItem>,
}

#[derive(Debug, Deserialize)]
struct EditorItem {
    index: usize,
    /// 挑战的认知编号（0-based, 999=无）
    thesis_index: usize,
    category: String,
    reason: String,
}

/// 幕僚长筛选：从 articles 中选出挑战现有认知的信息
///
/// world_state: 昨日简报摘要
/// theses: 当前世界模型的认知清单
pub async fn select_top_articles(
    articles: &[Article],
    world_state: &str,
    theses: &[String],
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<Vec<EditorSelection>> {
    if articles.is_empty() {
        return Ok(Vec::new());
    }

    // 1. 构建脱水输入（只传 title + category + 100 字摘要）
    let mut input = String::new();
    for (idx, art) in articles.iter().enumerate() {
        let desc = art
            .summary
            .as_deref()
            .or(art.content.as_deref())
            .unwrap_or("(无摘要)");
        // 截断到 100 字
        let end = desc.floor_char_boundary(100);
        let truncated: &str = if desc.len() > end { &desc[..end] } else { desc };
        input.push_str(&format!(
            "[{}] {} (分类: {})\n摘要: {}\n---\n",
            idx, art.title, art.category, truncated
        ));
    }

    let total = articles.len();
    log::info!("🎯 Editor Agent: {} 篇待筛选", total);

    // 2. 系统 Prompt（Thesis Challenge 核心）
    let theses_list: String = theses
        .iter()
        .enumerate()
        .map(|(i, t)| format!("  [{}] {}", i, t))
        .collect::<Vec<_>>()
        .join("\n");

    let system_prompt = format!(
        r#"你是一个个人创业者的 Chief of Staff（幕僚长）。你的职责不是找创业机会，而是判断每天的信息流中是否有任何一条**挑战了现有世界模型**。

## 当前世界模型（认知清单）

{}

## 当前世界状态（昨日简报）

{}

## 你的任务

对每条信息，判断它是否挑战上述认知清单中的任何一条：

- **挑战认知** = 这条信息如果为真，会让你重新思考某个认知的正确性
- **确认认知** = 这条信息支持某个认知，但没有挑战它
- **无关** = 这条信息与所有认知无关

## 输出规则

只输出 priority 为 HIGH 的条目（挑战认知），最多 3 条。

JSON 格式：
{{"selections": [
  {{"index": 序号, "thesis_index": 被挑战的认知编号(0-based), "category": "路由分类(AI/技术主线/创业/A股/芯片/政策)", "reason": "为什么它挑战了这个认知？（一句话，禁止用'值得关注'等废话）"}}
]}}

约束：
- priority 不是 HIGH 的不要输出
- 如果没有任何信息挑战现有认知 → 输出空数组
- 最多 3 条"#,
        theses_list, world_state
    );

    // 3. 调用 LLM（复用 llm::call_raw）
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let raw = llm::call_raw(&client, api_key, llm_config, &system_prompt, &input).await?;

    // 4. 解析响应 — 多策略容错
    let parsed = parse_editor_response(&raw)?;

    // 5. 按 index 匹配原文章，构建最终结果
    let mut selections = Vec::new();
    for item in parsed.selections {
        if let Some(article) = articles.get(item.index) {
            let thesis_label = if item.thesis_index < theses.len() {
                theses[item.thesis_index].as_str()
            } else {
                "(无)"
            };
            log::info!(
                "  🎯 挑战认知 [{}] {} → 认知:{} ({}): {}",
                item.index,
                article.title,
                item.thesis_index,
                thesis_label,
                item.reason
            );
            selections.push(EditorSelection {
                article: article.clone(),
                routed_category: item.category,
                thesis_index: item.thesis_index,
                reason: item.reason,
            });
        } else {
            log::warn!("⚠️ Editor 返回了越界 index {}，跳过", item.index);
        }
    }

    log::info!("🎯 Editor Agent: 选中 {}/{} 篇", selections.len(), total);
    Ok(selections)
}

/// 多策略解析 Editor 响应（兼容裸 JSON 和 Markdown 代码块）
fn parse_editor_response(raw: &str) -> Result<EditorResponse> {
    // 策略 1：直接解析
    if let Ok(parsed) = serde_json::from_str::<EditorResponse>(raw) {
        return Ok(parsed);
    }
    // 策略 2：提取 ```json ... ``` 块
    if let Some(block) = raw.split("```json\n").nth(1) {
        if let Some(json) = block.split("```").next() {
            if let Ok(parsed) = serde_json::from_str::<EditorResponse>(json.trim()) {
                return Ok(parsed);
            }
        }
    }
    // 策略 3：从第一个 { 到最后一个 }
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
