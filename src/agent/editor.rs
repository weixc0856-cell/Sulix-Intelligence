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
    category: String,
    reason: String,
}

/// 幕僚长筛选：从 articles 中选出最重要的 3-5 条
///
/// world_state: 昨日简报摘要，或首次运行时的默认提示
pub async fn select_top_articles(
    articles: &[Article],
    world_state: &str,
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

    // 2. 系统 Prompt
    let system_prompt = format!(
        r#"你是一个个人创业者的 Chief of Staff（幕僚长）。你的唯一职责是从今日信息流中选出 3-5 条最值得创始人关注的信号。

当前世界状态（昨天已覆盖的内容）：
{}

筛选标准：
- 只选会改变创始人决策或认知的信息
- 工具/插件/版本更新除非对创业假设有直接影响，否则忽略
- 不考虑已经覆盖过的重复话题
- 优先选范式级信号（S/A 级）> 重要信号（B 级）> 噪音（C 级）

输出 JSON 数组：
{{"selections": [{{"index": 序号, "category": "目标分类", "reason": "麦肯锡 So-What 理由（一句话，直接指出如何冲击或修正创始人的既定路线）"}}]}}

约束：
- 最多选 5 条
- ❌ 严禁在理由中使用"值得关注、不可否认、双刃剑、不可小觑、值得注意的是"等废话词
- ✍️ 理由必须直接指出该信息如何影响创业者的决策或路线图
- category 必须是已有分类之一：AI、技术主线、创业、A股、芯片、政策"#,
        world_state
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
            log::info!(
                "  🎯 选中 [{}] {} → {}: {}",
                item.index,
                article.title,
                item.category,
                item.reason
            );
            selections.push(EditorSelection {
                article: article.clone(),
                routed_category: item.category,
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
