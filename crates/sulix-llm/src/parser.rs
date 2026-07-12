//! LLM 响应 JSON 解析 — 多策略容错解析

use anyhow::Result;
use serde_json::Value;

use crate::types::{AnalyzedArticleRaw, ArticlesWrapper};

/// 多策略 JSON 解析（从 ArticlesWrapper 结构提权）
pub fn parse_json_response(content: &str) -> Result<Vec<AnalyzedArticleRaw>> {
    let val = parse_json_lenient(content)?;
    let wrapper: ArticlesWrapper = serde_json::from_value(val)?;
    Ok(wrapper.articles)
}

/// 多策略 JSON 解析（返回 Value，适合自定义字段提取）
///
/// 策略：直接解析 → 抽 ```json 围栏 → 抽 ``` 围栏 → 抓首尾花括号/方括号
pub fn parse_json_lenient(raw: &str) -> Result<Value> {
    // 策略 1: 直接解析
    if let Ok(v) = serde_json::from_str(raw) {
        return Ok(v);
    }
    // 策略 2: 提取 ```json ... ``` 块
    if let Some(inner) = extract_json_block(raw, "```json\n") {
        if let Ok(v) = serde_json::from_str(&inner) {
            return Ok(v);
        }
    }
    // 策略 3: 提取 ``` ... ``` 块（无 language hint）
    if let Some(inner) = extract_json_block(raw, "```\n") {
        if let Ok(v) = serde_json::from_str(&inner) {
            return Ok(v);
        }
    }
    // 策略 3b: 提取 ```json ... ``` 块（无 trailing \n）
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
    // 策略 4a: 从第一个 { 到最后一个 } 裸提取（对象）
    if let Some(start) = raw.find('{') {
        if let Some(end) = raw.rfind('}') {
            if end > start {
                if let Ok(v) = serde_json::from_str(&raw[start..=end]) {
                    return Ok(v);
                }
            }
        }
    }
    // 策略 4b: 从第一个 [ 到最后一个 ] 裸提取（数组）
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
pub fn extract_json_block(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)?;
    let after = &text[start + marker.len()..];
    let end = after.find("```")?;
    Some(after[..end].trim().to_string())
}

/// 从文本中提取标记之间的内容（灵活围栏，不要求 trailing \n）
pub fn extract_json_block_flexible(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)?;
    let after = &text[start + marker.len()..];
    let after = after.strip_prefix('\n').unwrap_or(after);
    let end = after.find("```")?;
    Some(after[..end].trim().to_string())
}

/// 从 LLM 响应文本中解析 JSON 数组。
#[allow(dead_code)]
pub fn parse_json_array<T: serde::de::DeserializeOwned>(raw: &str) -> Result<Vec<T>> {
    let val = parse_json_lenient(raw)?;
    let arr = val
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("expected JSON array, got {}", categorize_value(&val)))?;
    let mut result = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        result.push(
            serde_json::from_value(item.clone())
                .map_err(|e| anyhow::anyhow!("JSON array element {} parse error: {}", i, e))?,
        );
    }
    Ok(result)
}

/// Categorize a JSON value for error messages
#[allow(dead_code)]
fn categorize_value(v: &Value) -> &'static str {
    match v {
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "bool",
        Value::Null => "null",
    }
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
    fn test_parse_json_cjk() {
        let json = r#"{"articles":[{"title":"大模型商品化","importance":8,"relevance":"高","time_horizon":"短期","action":"研究","confidence":"中","judgment":"开源能力接近闭源"}]}"#;
        let result = parse_json_response(json).unwrap();
        assert_eq!(result[0].title, "大模型商品化");
    }
}
