//! Layer 2 Daily Intel 渲染器
//!
//! 输出 JSON 文件到 /intel/daily/（非 MDX，避免 YAML 转义风险）。
//! Astro 通过 data collection 消费 JSON，零 YAML 面。

use serde::Serialize;

/// Layer 2 每日情报条目
#[derive(Debug, Clone, Serialize)]
pub struct IntelEntry {
    pub id: String,
    pub title: String,
    pub date: String,
    pub source: String,
    pub url: String,
    pub domain: String,
    pub svi: u8,
    pub impact: String,
    pub summary: Option<String>,
    pub related_thesis: Option<String>,
}

/// 渲染单条情报为 JSON 字符串
pub fn render_intel_json(entry: &IntelEntry) -> String {
    serde_json::json!({
        "id": entry.id,
        "title": entry.title,
        "date": entry.date,
        "source": entry.source,
        "url": entry.url,
        "domain": entry.domain,
        "svi": entry.svi,
        "impact": entry.impact,
        "summary": entry.summary,
        "related_thesis": entry.related_thesis,
    })
    .to_string()
}

/// 渲染情报列表为 JSON 数组字符串
pub fn render_intel_list(entries: &[IntelEntry]) -> String {
    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "title": e.title,
                "date": e.date,
                "source": e.source,
                "domain": e.domain,
                "svi": e.svi,
                "impact": e.impact,
                "summary": e.summary,
            })
        })
        .collect();
    serde_json::to_string_pretty(&items).unwrap_or_default()
}
