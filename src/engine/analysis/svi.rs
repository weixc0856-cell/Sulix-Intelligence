//! SVI 评分 — calculate_svi + map_to_scl

use crate::domain::theme::{Theme, ThemeAnalysis};
use crate::config::SourceConfig;

// ===== SVI 权重常量 =====

const SVI_ENTITY_SURGE: f64 = 0.20;
const SVI_SANCTION_SENSITIVITY: f64 = 0.25;
const SVI_PATENT_MUTATION: f64 = 0.15;
const SVI_SOURCE_CREDIBILITY: f64 = 0.15;
const SVI_TEMPORAL_URGENCY: f64 = 0.10;
const SVI_RECENCY: f64 = 0.15;

// ===== SCL 置信等级映射 =====

/// 将 L1-L5 旧置信等级映射为 SCL（Sulix Confidence Level）
pub(super) fn map_to_scl(value: &str) -> String {
    match value.trim() {
        "L1" | "L2" | "确立" => "确立-事实".into(),
        "L3" | "发展中" => "发展中-推断".into(),
        "L4" | "建立" => "建立-传闻".into(),
        "L5" | "噪音" => "噪音".into(),
        other => other.to_string(),
    }
}

/// 计算战略异动指数 (SVI)，五维综合评分 0-10
pub fn calculate_svi(analysis: &ThemeAnalysis, theme: &Theme, sources: &[SourceConfig]) -> u8 {
    let article_count = theme.articles.len() as f64;
    let mut source_counts: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    for art in &theme.articles {
        *source_counts.entry(art.source.as_str()).or_insert(0) += 1;
    }
    let max_repeats = source_counts.values().copied().max().unwrap_or(1) as f64;
    let entity_surge = if article_count >= 3.0 {
        (max_repeats / article_count).min(1.0)
    } else {
        0.3
    };

    let best_score = theme
        .articles
        .iter()
        .map(|a| {
            sources
                .iter()
                .find(|s| s.name == a.source)
                .map(|s| s.score as f64 / 10.0)
                .unwrap_or(0.5)
        })
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.5);

    let recency = if let Some(pub_date) = theme.articles.iter().filter_map(|a| a.published_at).max()
    {
        let days_old = (chrono::Utc::now() - pub_date.with_timezone(&chrono::Utc))
            .num_days()
            .max(0);
        if days_old <= 1 {
            1.0
        } else if days_old <= 3 {
            0.8
        } else if days_old <= 7 {
            0.5
        } else {
            0.2
        }
    } else {
        0.5
    };

    let temporal_urgency = (analysis.signal_strength as f64) / 10.0;
    let sanction_sensitivity = temporal_urgency;
    let patent_mutation = (analysis.signal_strength as f64) / 10.0;

    let score = entity_surge * SVI_ENTITY_SURGE
        + sanction_sensitivity * SVI_SANCTION_SENSITIVITY
        + patent_mutation * SVI_PATENT_MUTATION
        + best_score * SVI_SOURCE_CREDIBILITY
        + temporal_urgency * SVI_TEMPORAL_URGENCY
        + recency * SVI_RECENCY;

    (score * 10.0).round().clamp(0.0, 10.0) as u8
}
