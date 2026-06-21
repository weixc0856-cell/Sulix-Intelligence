//! LLM 分析模块 — DeepSeek API 调用
//!
//! 将新增文章按 vertical 分组，每组调用 DeepSeek 做结构化分析，
//! 返回解析后的 AnalyzedArticle 列表。
//!
//! 核心升级（P1）：
//! - 分批策略：每个 vertical 超过 BATCH_SIZE 篇自动拆分
//! - 重试机制：指数退避（1s → 2s → 4s），最多 3 次

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::{LlmConfig, PromptConfig};
use crate::fetcher::Article;

/// 每批最多 8 篇文章（控制 token 消耗 + 保证每篇的 attention）
const BATCH_SIZE: usize = 8;

/// 最大重试次数
const MAX_RETRIES: u32 = 3;

/// 单个 vertical 的分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerticalAnalysis {
    pub category: String,
    pub articles: Vec<AnalyzedArticle>,
}

/// 分析后的文章（支持红蓝对抗：judgment=红军叙事，blue_rebuttal=蓝军反驳，summary=一句话核心）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedArticle {
    pub title: String,
    pub url: String,
    pub importance: u8,
    pub relevance: String,
    pub time_horizon: String,
    pub action: String,
    pub confidence: String,
    pub judgment: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub blue_rebuttal: String,
    #[serde(default)]
    pub arbitration: String,
}

/// 按 category 将文章分组
pub fn group_by_category(articles: &[Article]) -> HashMap<String, Vec<Article>> {
    let mut grouped: HashMap<String, Vec<Article>> = HashMap::new();
    for article in articles {
        grouped
            .entry(article.category.clone())
            .or_default()
            .push(article.clone());
    }
    grouped
}

/// 分析所有 vertical 的文章（支持分批 + 重试）
pub async fn analyze(
    grouped: &HashMap<String, Vec<Article>>,
    prompts: &PromptConfig,
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<Vec<VerticalAnalysis>> {
    if grouped.is_empty() {
        return Ok(Vec::new());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let mut all_analyses = Vec::new();

    for (category, articles) in grouped {
        log::info!(
            "🔍 正在分析 [{}] — {} 篇 (分 {} 批)",
            category,
            articles.len(),
            articles.len().div_ceil(BATCH_SIZE)
        );

        let system_prompt = build_system_prompt(prompts, category);
        let mut batch_results = Vec::new();

        for (batch_idx, batch) in articles.chunks(BATCH_SIZE).enumerate() {
            if articles.len() > BATCH_SIZE {
                log::info!(
                    "  ↳ 第 {}/{} 批 ({} 篇)",
                    batch_idx + 1,
                    articles.len().div_ceil(BATCH_SIZE),
                    batch.len()
                );
            }

            let user_prompt = build_user_prompt(category, batch_idx + 1, batch);

            let result =
                call_with_retry(&client, api_key, llm_config, &system_prompt, &user_prompt).await;

            match result {
                Ok(analyzed) => {
                    let enriched = enrich_with_urls(analyzed, batch);
                    batch_results.extend(enriched);
                }
                Err(e) => {
                    log::warn!(
                        "⚠️ [{}] 第{}批分析失败 ({} 次重试后): {}",
                        category,
                        batch_idx + 1,
                        MAX_RETRIES,
                        e
                    );
                    // 该批降级：生成原始条目
                    for a in batch {
                        batch_results.push(AnalyzedArticle {
                            title: a.title.clone(),
                            url: a.url.clone(),
                            importance: 5,
                            relevance: "未分析".into(),
                            time_horizon: "未分析".into(),
                            action: "未分析".into(),
                            confidence: "低".into(),
                            judgment: format!("⚠️ LLM 分析失败，原文: {}", a.url),
                            summary: String::new(),
                            blue_rebuttal: String::new(),
                            arbitration: String::new(),
                        });
                    }
                }
            }

            // 批间间隔
            if articles.len() > BATCH_SIZE {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
        }

        log::info!("✅ [{}] 分析完成: {} 条判断", category, batch_results.len());
        all_analyses.push(VerticalAnalysis {
            category: category.clone(),
            articles: batch_results,
        });
    }

    Ok(all_analyses)
}

/// 构建 system prompt：base + vertical override + JSON 格式约束
fn build_system_prompt(prompts: &PromptConfig, category: &str) -> String {
    let mut prompt = prompts.base.clone();

    if let Some(override_text) = prompts.vertical_overrides.get(category) {
        prompt.push_str("\n\n");
        prompt.push_str(override_text);
    }

    prompt.push_str(
        "\n\n你必须以 JSON 格式输出。格式如下（严格遵循，不要加 markdown 代码块标记）：\n\
        {\n  \
        \"articles\": [\n    \
        {\n      \
        \"id\": \"文章的 ID（从输入原文获取，严格保持原样）\",\n      \
        \"summary\": \"一句话核心摘要（30-50字，大白话，去掉水话）\",\n      \
        \"title\": \"文章标题\",\n      \
        \"importance\": 7,\n      \
        \"relevance\": \"高/中/低\",\n      \
        \"time_horizon\": \"短期/中期/长期\",\n      \
        \"action\": \"立即行动/研究/观察/忽略\",\n      \
        \"confidence\": \"高/中/低\",\n      \
        \"judgment\": \"核心解读（2-4 句话）\"\n    \
        }\n  \
        ]\n\
        }\n\n\
       注意事项：\n\
        1. summary 必须是一句话（30-50字），用大白话写出核心信息，不要水话和修饰\n\
        2. importance 必须是 1-10 的整数\n\
        3. relevance、time_horizon、action、confidence 必须使用指定的枚举值\n\
        4. judgment 必须包含判断逻辑和从创业者视角的解读\n\
        5. 为每篇输入文章都生成一条分析结果，数量严格对应\n\
        6. id 字段必须从输入原文中获取并严格保持原样\n\
        7. 输出纯 JSON，不要在前后加任何说明文字",
    );

    prompt
}

/// 构建 user prompt：将所有文章格式化发给 LLM
fn build_user_prompt(category: &str, batch_idx: usize, articles: &[Article]) -> String {
    let mut prompt = format!(
        "请分析以下 {} 领域的 {} 条新闻（第 {} 批），输出 JSON：\n\n",
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

        // 截取前 3000 字符（相比原来 800 大幅提升，给 LLM 足够上下文）
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

// ===== P1: 重试机制 =====

/// 带指数退避重试的 API 调用
pub(crate) async fn call_with_retry(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<Vec<AnalyzedArticleRaw>> {
    let mut last_error = None;

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay_secs = 2u64.pow(attempt); // 1s, 2s, 4s
            log::warn!("⏳ 第 {} 次重试 ({}s 后)...", attempt, delay_secs);
            tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
        }

        match call_deepseek(client, api_key, llm_config, system_prompt, user_prompt).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                let err_str = e.to_string();

                // 4xx 错误不重试（auth/billing/rate limit 非临时性问题）
                if err_str.contains("401") || err_str.contains("403") || err_str.contains("429") {
                    log::warn!("❌ 非临时性错误，不重试: {}", err_str);
                    return Err(e);
                }

                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap())
}

/// 实际调用 DeepSeek API
async fn call_deepseek(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<Vec<AnalyzedArticleRaw>> {
    let url = format!(
        "{}/chat/completions",
        llm_config.base_url.trim_end_matches('/')
    );

    let request_body = serde_json::json!({
        "model": llm_config.model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "max_tokens": llm_config.max_tokens,
        "temperature": llm_config.temperature,
        "response_format": {"type": "json_object"}
    });

    log::debug!("LLM 请求: {} ({} tokens max)", url, llm_config.max_tokens);

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "DeepSeek API 返回错误 ({}): {}",
            status,
            error_text
        ));
    }

    let chat_response: ChatResponse = response.json().await?;

    let content = chat_response
        .choices
        .first()
        .map(|c| &c.message.content)
        .ok_or_else(|| anyhow::anyhow!("API 响应中没有 choices"))?
        .clone();

    parse_json_response(&content).map_err(|e| {
        let end = content.floor_char_boundary(content.len().min(100));
        anyhow::anyhow!("JSON 解析失败 ({}): {}...", e, &content[..end])
    })
}

/// 多策略 JSON 解析
fn parse_json_response(content: &str) -> Result<Vec<AnalyzedArticleRaw>> {
    // 策略 1：直接解析
    if let Ok(parsed) = serde_json::from_str::<ArticlesWrapper>(content) {
        return Ok(parsed.articles);
    }

    // 策略 2：提取 ```json ... ``` 块
    if let Some(json_str) = extract_json_block(content, "```json\n") {
        if let Ok(parsed) = serde_json::from_str::<ArticlesWrapper>(&json_str) {
            return Ok(parsed.articles);
        }
    }

    // 策略 3：提取 ``` ... ``` 块
    if let Some(json_str) = extract_json_block(content, "```\n") {
        if let Ok(parsed) = serde_json::from_str::<ArticlesWrapper>(&json_str) {
            return Ok(parsed.articles);
        }
    }

    // 策略 4：从第一个 { 到最后一个 } 裸提取
    if let Some(start) = content.find('{') {
        if let Some(end) = content.rfind('}') {
            if end > start {
                let sliced = &content[start..=end];
                if let Ok(parsed) = serde_json::from_str::<ArticlesWrapper>(sliced) {
                    return Ok(parsed.articles);
                }
            }
        }
    }

    Err(anyhow::anyhow!("所有 JSON 解析策略均失败"))
}

/// 从文本中提取指定标记之间的内容
fn extract_json_block(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)?;
    let after = &text[start + marker.len()..];
    let end = after.find("```")?;
    Some(after[..end].trim().to_string())
}

/// 将 LLM 返回的分析结果与原文章关联（补全 url）
/// 优先按 id 匹配，其次是 title（向后兼容）
fn enrich_with_urls(
    analyzed: Vec<AnalyzedArticleRaw>,
    original_articles: &[Article],
) -> Vec<AnalyzedArticle> {
    let id_url_map: HashMap<&str, &str> = original_articles
        .iter()
        .map(|a| (a.id.as_str(), a.url.as_str()))
        .collect();
    let title_url_map: HashMap<&str, &str> = original_articles
        .iter()
        .map(|a| (a.title.as_str(), a.url.as_str()))
        .collect();

    analyzed
        .into_iter()
        .map(|raw| {
            // 先按 id 匹配，失败则按 title 匹配（LLM 改标题时仍能工作）
            let url = if !raw.id.is_empty() {
                id_url_map
                    .get(raw.id.as_str())
                    .copied()
                    .unwrap_or("")
                    .to_string()
            } else {
                title_url_map
                    .get(raw.title.as_str())
                    .copied()
                    .unwrap_or("")
                    .to_string()
            };

            AnalyzedArticle {
                title: raw.title,
                url,
                importance: raw.importance.clamp(1, 10),
                relevance: raw.relevance,
                time_horizon: raw.time_horizon,
                action: raw.action,
                confidence: raw.confidence,
                judgment: raw.judgment,
                summary: String::new(),
                blue_rebuttal: String::new(),
                arbitration: String::new(),
            }
        })
        .collect()
}

// ========== 内部数据结构 ==========

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct ArticlesWrapper {
    articles: Vec<AnalyzedArticleRaw>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnalyzedArticleRaw {
    #[serde(default)]
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) importance: u8,
    pub(crate) relevance: String,
    pub(crate) time_horizon: String,
    pub(crate) action: String,
    pub(crate) confidence: String,
    pub(crate) judgment: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_direct() {
        let json = r#"{"articles":[{"title":"Test","importance":7,"relevance":"高","time_horizon":"短期","action":"研究","confidence":"中","judgment":"测试"}]}"#;
        let result = parse_json_response(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Test");
    }

    #[test]
    fn test_parse_json_codeblock() {
        let json = "text\n```json\n{\"articles\":[{\"title\":\"CodeBlock\",\"importance\":5,\"relevance\":\"中\",\"time_horizon\":\"短期\",\"action\":\"观察\",\"confidence\":\"低\",\"judgment\":\"test\"}]}\n```\nmore";
        let result = parse_json_response(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "CodeBlock");
    }

    #[test]
    fn test_parse_json_bare_codeblock() {
        let json = "```\n{\"articles\":[{\"title\":\"Bare\",\"importance\":3,\"relevance\":\"低\",\"time_horizon\":\"短期\",\"action\":\"忽略\",\"confidence\":\"低\",\"judgment\":\"bare\"}]}\n```";
        let result = parse_json_response(json).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_json_brace_extract() {
        let json = "prefix\n{\"articles\":[{\"title\":\"Extract\",\"importance\":6,\"relevance\":\"高\",\"time_horizon\":\"中期\",\"action\":\"研究\",\"confidence\":\"中\",\"judgment\":\"extract\"}]}\nsuffix";
        let result = parse_json_response(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Extract");
    }

    #[test]
    fn test_parse_json_invalid() {
        let result = parse_json_response("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_json_empty_array() {
        let json = r#"{"articles":[]}"#;
        let result = parse_json_response(json).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_json_block_normal() {
        let result = extract_json_block(
            "before\n```json\n{\"key\":\"val\"}\n```\nafter",
            "```json\n",
        );
        assert_eq!(result, Some("{\"key\":\"val\"}".into()));
    }

    #[test]
    fn test_extract_json_block_no_end() {
        let result = extract_json_block("before\n```json\n{\"key\":\"val\"}", "```json\n");
        assert_eq!(result, None);
    }

    #[test]
    fn test_group_by_category_empty() {
        let result = group_by_category(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_group_by_category_multiple() {
        use crate::fetcher::Article;
        let a1 = Article {
            id: "1".into(),
            source: "s".into(),
            title: "A".into(),
            url: "u1".into(),
            content: None,
            summary: None,
            published_at: None,
            category: "AI".into(),
        };
        let a2 = Article {
            id: "2".into(),
            source: "s".into(),
            title: "B".into(),
            url: "u2".into(),
            content: None,
            summary: None,
            published_at: None,
            category: "创业".into(),
        };
        let a3 = Article {
            id: "3".into(),
            source: "s".into(),
            title: "C".into(),
            url: "u3".into(),
            content: None,
            summary: None,
            published_at: None,
            category: "AI".into(),
        };
        let grouped = group_by_category(&[a1, a2, a3]);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped.get("AI").unwrap().len(), 2);
        assert_eq!(grouped.get("创业").unwrap().len(), 1);
    }
}
