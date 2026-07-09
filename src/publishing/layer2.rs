//! Layer 2 — Daily Intel 轻量发布器
//!
//! 接收来自 classify_and_route 的 SignalAssessment（score ≥ 3），
//! 输出 JSON 到 /intel/daily/。
//! Phase 0: 仅输出标题/来源/链接（无 LLM 摘要）。

use anyhow::Result;
use std::path::Path;

use crate::agent::scan::SignalAssessment;
use crate::renderer::intel::IntelEntry;
use crate::renderer::intel::render_intel_json;

/// 发布 Layer 2 intel 条目到 /intel/daily/{date}-{slug}.json
/// 返回已发布数量
pub fn publish_intel(
    assessments: &[SignalAssessment],
    today: &str,
    intel_dir: &Path,
) -> Result<usize> {
    let mut published = 0usize;

    std::fs::create_dir_all(intel_dir)?;

    for assessment in assessments {
        let summary = None; // Phase 0: 无 LLM 摘要

        let impact = if assessment.score >= 7 { "high" }
            else if assessment.score >= 5 { "medium" }
            else { "low" };

        let entry = IntelEntry {
            id: format!("intel-{}-{}", today, assessment.article_id),
            title: assessment.title.clone(),
            date: today.to_string(),
            source: assessment.source.clone(),
            url: assessment.url.clone(),
            domain: assessment.domain.clone(),
            svi: assessment.score,
            impact: impact.to_string(),
            summary,
            related_thesis: None,
        };

        let slug = slugify(&assessment.title, &assessment.article_id);
        let path = intel_dir.join(format!("{}-{}.json", today, slug));
        let json = render_intel_json(&entry);
        match std::fs::write(&path, &json) {
            Ok(_) => published += 1,
            Err(e) => log::warn!("⚠️ Layer2 intel write failed [{}]: {}", slug, e),
        }
    }

    log::info!("📰 Layer 2: {} intel items published to {}", published, intel_dir.display());
    Ok(published)
}

/// 简单的 slug 生成
///
/// article_id 尾部拼入防止碰撞（article_id 已是 URL hash，无需重算 sha256）。
fn slugify(title: &str, article_id: &str) -> String {
    let slug: String = title.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == ' ')
        .take(40)
        .collect::<String>()
        .trim()
        .to_lowercase()
        .replace(' ', "-");
    let suffix: String = article_id.chars()
        .filter(|c| c.is_alphanumeric())
        .rev()
        .take(8)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if slug.is_empty() {
        suffix
    } else {
        format!("{}-{}", slug, suffix)
    }
}
