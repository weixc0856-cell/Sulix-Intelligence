//! LLM 分析模块 — 多 Provider 调用
//!
//! 将新增文章按 vertical 分组，每组调用 LLM 做结构化分析，
//! 返回解析后的 AnalyzedArticle 列表。
//!
//! 核心升级（P1）：
//! - 分批策略：每个 vertical 超过 BATCH_SIZE 篇自动拆分
//! - 重试机制：指数退避（1s → 2s → 4s），最多 3 次

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::config::LlmConfig;
use crate::fetcher::Article;

/// 最大重试次数
const MAX_RETRIES: u32 = 3;

/// Create a reqwest Client with the given timeout in seconds.
pub fn create_client(timeout_secs: u64) -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()?)
}

/// Convenience: LLM API calls with 30-second timeout.
pub fn create_llm_client() -> Result<reqwest::Client> {
    create_client(30)
}

/// Convenience: external source fetches with 60-second timeout.
pub fn create_source_client() -> Result<reqwest::Client> {
    create_client(60)
}

// ===== LLM 调用审计计数器 =====
/// 总调用次数
pub static LLM_CALL_COUNT: AtomicU64 = AtomicU64::new(0);
/// 估计输入 token 数（字符数 / 4 粗略估计）
pub static LLM_INPUT_TOKENS: AtomicU64 = AtomicU64::new(0);
/// 估计输出 token 数
pub static LLM_OUTPUT_TOKENS: AtomicU64 = AtomicU64::new(0);

/// 获取 LLM 审计统计摘要
pub fn llm_audit_summary() -> String {
    let calls = LLM_CALL_COUNT.load(Ordering::Relaxed);
    let input = LLM_INPUT_TOKENS.load(Ordering::Relaxed);
    let output = LLM_OUTPUT_TOKENS.load(Ordering::Relaxed);
    format!(
        "LLM 调用: {} 次, 输入 ~{}k tokens, 输出 ~{}k tokens",
        calls,
        input / 1000,
        output / 1000,
    )
}

/// 单个 vertical 的分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerticalAnalysis {
    pub category: String,
    pub articles: Vec<AnalyzedArticle>,
}

/// 分析后的文章（支持红蓝对抗：strategic_level=S/A/B/C）
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
    pub strategic_level: String,
    #[serde(default)]
    pub blue_rebuttal: String,
    #[serde(default)]
    pub arbitration: String,
    #[serde(default)]
    pub evidence_type: String,
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

// ===== P1: 重试机制 =====

/// Generic retry loop with exponential backoff.
/// Skips retry on 4xx errors (auth/billing/rate-limit).
async fn with_retry<T, F, Fut>(f: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = None;
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay_secs = 2u64.pow(attempt);
            log::warn!("⏳ Retry attempt {} ({}s delay)...", attempt, delay_secs);
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        }
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("401") || err_str.contains("403") {
                    log::warn!("❌ Non-retryable error: {}", err_str);
                    return Err(e);
                }
                last_error = Some(e);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("retry loop exited without error accumulation")))
}

/// 带指数退避重试的 API 调用
pub(crate) async fn call_with_retry(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<Vec<AnalyzedArticleRaw>> {
    with_retry(|| call_completion(client, api_key, llm_config, system_prompt, user_prompt)).await
}

/// 调用 LLM API 返回原始文本，带指数退避重试
pub(crate) async fn call_with_retry_raw(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    with_retry(|| call_raw_inner(client, api_key, llm_config, system_prompt, user_prompt)).await
}

/// Simple text-in/text-out LLM call (creates its own client).
/// Used by lightweight classification tasks that don't need JSON parsing.
pub(crate) async fn call_and_parse(
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    call_with_retry_raw(&client, api_key, llm_config, system_prompt, user_prompt).await
}

/// Core LLM call: build request, send, extract content string.
/// Returns the raw content string from the LLM response.
/// All public/crate callers go through with_retry wrappers.
async fn call_llm_inner(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
    max_tokens: u32,
    temperature: f32,
) -> Result<String> {
    LLM_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
    let input_est = (system_prompt.len() + user_prompt.len()) as u64;
    LLM_INPUT_TOKENS.fetch_add(input_est / 4, Ordering::Relaxed);

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
        "max_tokens": max_tokens,
        "temperature": temperature,
        "response_format": {"type": "json_object"},
        "seed": 42  // fixed seed for deterministic output
    });

    log::debug!("LLM 请求: {} ({} tokens max)", url, max_tokens);

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<body read failed: {}>", e));
        return Err(anyhow::anyhow!("LLM API 返回错误 ({}): {}", status, error_text));
    }

    let chat_response: ChatResponse = response.json().await?;
    let content = chat_response
        .choices
        .first()
        .map(|c| &c.message.content)
        .ok_or_else(|| anyhow::anyhow!("API 响应中没有 choices"))?
        .clone();

    LLM_OUTPUT_TOKENS.fetch_add(content.len() as u64 / 4, Ordering::Relaxed);

    Ok(content)
}

/// 不带重试的原始 API 调用 — 返回文本（供 call_with_retry_raw 使用）
async fn call_raw_inner(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    call_llm_inner(
        client, api_key, llm_config, system_prompt, user_prompt,
        llm_config.max_tokens.min(2048),
        llm_config.temperature.min(0.2),
    ).await
}

/// 实际调用 LLM API 并解析 JSON 响应（供 call_with_retry 使用）
async fn call_completion(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<Vec<AnalyzedArticleRaw>> {
    let content = call_llm_inner(
        client, api_key, llm_config, system_prompt, user_prompt,
        llm_config.max_tokens,
        llm_config.temperature,
    ).await?;

    parse_json_response(&content).map_err(|e| {
        let end = content.floor_char_boundary(content.len().min(100));
        anyhow::anyhow!("JSON 解析失败 ({}): {}...", e, &content[..end])
    })
}

/// 多策略 JSON 解析
fn parse_json_response(content: &str) -> Result<Vec<AnalyzedArticleRaw>> {
    let val = parse_json_lenient(content)?;
    let wrapper: ArticlesWrapper = serde_json::from_value(val)?;
    Ok(wrapper.articles)
}

/// 多策略 JSON 解析（返回 Value，适合自定义字段提取）
/// 策略：直接解析 → 抽 ```json 围栏 → 抽 ``` 围栏 → 抓首尾花括号/方括号
pub(crate) fn parse_json_lenient(raw: &str) -> Result<serde_json::Value> {
    // 策略 1：直接解析
    if let Ok(v) = serde_json::from_str(raw) {
        return Ok(v);
    }
    // 策略 2：提取 ```json ... ``` 块
    if let Some(inner) = extract_json_block(raw, "```json\n") {
        if let Ok(v) = serde_json::from_str(&inner) {
            return Ok(v);
        }
    }
    // 策略 3：提取 ``` ... ``` 块（无 language hint）
    if let Some(inner) = extract_json_block(raw, "```\n") {
        if let Ok(v) = serde_json::from_str(&inner) {
            return Ok(v);
        }
    }
    // 策略 3b: 提取 ```json ... ``` 块（无 trailing \n — leftover after trim)
    if let Some(inner) = extract_json_block_flexible(raw, "```json") {
        if let Ok(v) = serde_json::from_str(&inner) {
            return Ok(v);
        }
    }
    // 策略 3c: 提取 ``` ...  ``` 块（无 trailing \n）
    if let Some(inner) = extract_json_block_flexible(raw, "```") {
        if let Ok(v) = serde_json::from_str(&inner) {
            return Ok(v);
        }
    }
    // 策略 4a：从第一个 { 到最后一个 } 裸提取（对象）
    if let Some(start) = raw.find('{') {
        if let Some(end) = raw.rfind('}') {
            if end > start {
                if let Ok(v) = serde_json::from_str(&raw[start..=end]) {
                    return Ok(v);
                }
            }
        }
    }
    // 策略 4b：从第一个 [ 到最后一个 ] 裸提取（数组）
    if let Some(start) = raw.find('[') {
        if let Some(end) = raw.rfind(']') {
            if end > start {
                if let Ok(v) = serde_json::from_str(&raw[start..=end]) {
                    return Ok(v);
                }
            }
        }
    }
    Err(anyhow::anyhow!("所有 JSON 解析策略均失败"))
}

/// 从文本中提取指定标记之间的内容（严格围栏，需要 trailing \n）
pub(crate) fn extract_json_block(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)?;
    let after = &text[start + marker.len()..];
    let end = after.find("```")?;
    Some(after[..end].trim().to_string())
}

/// 从文本中提取标记之间的内容（灵活围栏，不要求 trailing \n）
/// 处理 LLM 可能输出的 ```json\n...``` 或 ```json...``` 两种格式
pub(crate) fn extract_json_block_flexible(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)?;
    let after = &text[start + marker.len()..];
    // Skip optional newline after marker
    let after = after.strip_prefix('\n').unwrap_or(after);
    let end = after.find("```")?;
    Some(after[..end].trim().to_string())
}

/// 从 LLM 响应文本中解析 JSON 数组。
/// 使用 parse_json_lenient 提取 Value，然后尝试转为数组。
pub(crate) fn parse_json_array<T: serde::de::DeserializeOwned>(raw: &str) -> Result<Vec<T>> {
    let val = parse_json_lenient(raw)?;
    let arr = val.as_array()
        .ok_or_else(|| anyhow::anyhow!("expected JSON array, got {}", categorize_value(&val)))?;
    // Try to deserialize each element independently for better error messages
    let mut result = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        result.push(serde_json::from_value(item.clone())
            .map_err(|e| anyhow::anyhow!("JSON array element {} parse error: {}", i, e))?);
    }
    Ok(result)
}

/// Categorize a JSON value for error messages
fn categorize_value(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Null => "null",
    }
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

/// Raw LLM response struct — only fields consumed downstream.
/// Serde ignores unknown fields by default, so extra API fields are harmless.
#[derive(Debug, Deserialize)]
pub(crate) struct AnalyzedArticleRaw {
    pub(crate) title: String,
    pub(crate) importance: u8,
    pub(crate) relevance: String,
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
            wiki_summary: None,
            evidence_type: String::new(),
            is_internal: false,
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
            wiki_summary: None,
            evidence_type: String::new(),
            is_internal: false,
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
            wiki_summary: None,
            evidence_type: String::new(),
            is_internal: false,
        };
        let grouped = group_by_category(&[a1, a2, a3]);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped.get("AI").unwrap().len(), 2);
        assert_eq!(grouped.get("创业").unwrap().len(), 1);
    }

    #[test]
    fn test_parse_json_cjk() {
        let json = r#"{"articles":[{"title":"大模型商品化","importance":8,"relevance":"高","time_horizon":"短期","action":"研究","confidence":"中","judgment":"开源能力接近闭源"}]}"#;
        let result = parse_json_response(json).unwrap();
        assert_eq!(result[0].title, "大模型商品化");
    }
}
