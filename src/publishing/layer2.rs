//! Layer 2 — Daily Intel 轻量发布器
//!
//! 接收来自 Scan Agent 分诊后的文章（score ≥ 3），输出 JSON 到 /intel/daily/。
//! Phase 0: 仅输出标题/来源/链接（无 LLM 摘要）。
//! Phase 1+: 按 SVI ≥ 5 触发 LLM 压缩摘要。

use anyhow::Result;
use std::path::Path;

use crate::fetcher::Article;
use crate::renderer::intel::IntelEntry;
use crate::renderer::intel::render_intel_json;

/// 发布 Layer 2 intel 条目到 /intel/daily/{date}-{slug}.json
/// 返回已发布数量
pub fn publish_intel(
    articles: &[Article],
    today: &str,
    intel_dir: &Path,
) -> Result<usize> {
    let mut published = 0usize;

    // 直接使用 Article 的 summary 字段作为摘要（如果有）
    // Phase 0: 无额外 LLM 调用
    for article in articles {
        let summary = article.summary.clone().or_else(|| {
            // 从 content 取前 200 字符作为粗摘要（无 LLM）
            article.content.as_ref().map(|c| {
                c.chars().take(200).collect::<String>()
            })
        });

        let entry = IntelEntry {
            id: format!("intel-{}-{}", today, article.id),
            title: article.title.clone(),
            date: today.to_string(),
            source: article.source.clone(),
            url: article.url.clone(),
            domain: article.category.clone(),
            svi: 0,  // Phase 0: 暂无 svi，未来从 Scan result 传入
            impact: "low".to_string(),
            summary,
            related_thesis: None,
        };

        let slug = slugify(&article.title);
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
fn slugify(title: &str) -> String {
    title.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == ' ')
        .take(40)
        .collect::<String>()
        .trim()
        .to_lowercase()
        .replace(' ', "-")
}
