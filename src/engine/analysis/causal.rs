//! 因果链解析 — parse_causal_chain

use crate::domain::theme::CausalChain;

/// 解析 LLM 输出的因果链字符串
pub(super) fn parse_causal_chain(value: &serde_json::Value) -> Vec<CausalChain> {
    let text = match value.as_str() {
        Some(s) if !s.is_empty() && s != "null" => s.to_string(),
        _ => return vec![],
    };
    let parts: Vec<&str> = text
        .split(|c| c == '→' || (c == '-' && text.contains("->")))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() < 2 {
        return vec![];
    }
    let trigger = parts[0].to_string();
    let direct_effect = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
    let chain_reaction: Vec<String> = parts.iter().skip(2).map(|s| s.to_string()).collect();
    vec![CausalChain {
        trigger,
        direct_effect,
        chain_reaction,
        second_order: vec![],
    }]
}
