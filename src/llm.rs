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

use crate::config::LlmConfig;
use crate::fetcher::Article;

/// 最大重试次数
const MAX_RETRIES: u32 = 3;

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

        match call_completion(client, api_key, llm_config, system_prompt, user_prompt).await {
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

/// 调用 DeepSeek API 返回原始文本，带指数退避重试
pub(crate) async fn call_with_retry_raw(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let mut last_error = None;
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay_secs = 2u64.pow(attempt);
            log::warn!("⏳ 第 {} 次重试 ({}s 后)...", attempt, delay_secs);
            tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
        }
        match call_raw_inner(client, api_key, llm_config, system_prompt, user_prompt).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                let err_str = e.to_string();
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

/// 不带重试的原始 API 调用（供 call_with_retry_raw 使用）
async fn call_raw_inner(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
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
        "max_tokens": llm_config.max_tokens.min(2048),
        "temperature": llm_config.temperature.min(0.2),
        "response_format": {"type": "json_object"}
    });

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

    Ok(content)
}

/// 实际调用 LLM API（OpenAI 兼容格式，由 config.base_url 决定 provider）
async fn call_completion(
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
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<body read failed: {}>", e));
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
    let val = parse_json_lenient(content)?;
    let wrapper: ArticlesWrapper = serde_json::from_value(val)?;
    Ok(wrapper.articles)
}

/// 多策略 JSON 解析（返回 Value，适合自定义字段提取）
/// 策略：直接解析 → 抽 ```json 围栏 → 抽 ``` 围栏 → 抓首尾花括号
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
    // 策略 3：提取 ``` ... ``` 块
    if let Some(inner) = extract_json_block(raw, "```\n") {
        if let Ok(v) = serde_json::from_str(&inner) {
            return Ok(v);
        }
    }
    // 策略 4：从第一个 { 到最后一个 } 裸提取
    if let Some(start) = raw.find('{') {
        if let Some(end) = raw.rfind('}') {
            if end > start {
                if let Ok(v) = serde_json::from_str(&raw[start..=end]) {
                    return Ok(v);
                }
            }
        }
    }
    Err(anyhow::anyhow!("所有 JSON 解析策略均失败"))
}

/// 从文本中提取指定标记之间的内容
pub(crate) fn extract_json_block(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)?;
    let after = &text[start + marker.len()..];
    let end = after.find("```")?;
    Some(after[..end].trim().to_string())
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
#[allow(dead_code)]
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
