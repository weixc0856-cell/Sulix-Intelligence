//! ArtifactSet 构造器 — publishing 内部调用的纯函数
//!
//! 不是平行入口，由 publishing::agent_publish 调用。

use crate::domain::artifact::ArtifactSet;
use crate::domain::theme::{Theme, ThemeAnalysis};
use crate::domain::ThesisDecision;
use crate::domain::EditorNote;
use crate::engine::memory::MemoryEngine;
use crate::domain::premium::PremiumReport;
use crate::event_log::ObjectEvent;
use std::collections::HashMap;
use std::path::Path;

/// 从 publishing 输出构建 ArtifactSet
#[allow(clippy::too_many_arguments)]
pub fn build_artifact_set(
    themes: Vec<Theme>,
    analyses: Vec<ThemeAnalysis>,
    analyses_zh: Vec<ThemeAnalysis>,
    memory: MemoryEngine,
    thesis_decisions: Vec<ThesisDecision>,
    premium_reports: Vec<PremiumReport>,
    editor_notes: Vec<EditorNote>,
    investigation_reports: Vec<(String, crate::domain::investigation::InvestigationReport, Option<String>, Option<String>)>,
    new_articles: Vec<crate::fetcher::Article>,
    events: Vec<ObjectEvent>,
    today: &str,
    asi_score_map: HashMap<String, (f64, f64, f64)>,
    belief_notes_html: String,
    refined_domains: HashMap<String, (crate::domain::StrategicDomain, Vec<crate::domain::StrategicDomain>)>,
    mdx_path: &Path,
) -> ArtifactSet {
    // 统计计数（供 manifest 初步使用，但 manifest 最终在 delivery 验证门后从 ArtifactSet 读取）
    let count_md = |dir: &Path| -> usize {
        std::fs::read_dir(dir)
            .map(|d| d.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                .count())
            .unwrap_or(0)
    };

    let assessment_count = count_md(&mdx_path.join("thesis"));
    let investigation_count = count_md(&mdx_path.join("investigation"));
    let total_signals = count_md(&mdx_path.join("daily"));

    // archive_days: unique dates in daily/
    let archive_days = std::fs::read_dir(mdx_path.join("daily"))
        .map(|d| {
            let dates: std::collections::HashSet<String> = d
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    e.file_name().to_str()
                        .and_then(|n| n.get(..10))
                        .filter(|s| s.chars().nth(4) == Some('-'))
                        .map(|s| s.to_string())
                })
                .collect();
            dates.len()
        })
        .unwrap_or(0);

    ArtifactSet::new(
        themes, analyses, analyses_zh,
        memory, thesis_decisions, premium_reports,
        editor_notes, investigation_reports, new_articles,
        events, today.to_string(), asi_score_map, belief_notes_html, refined_domains,
        assessment_count, investigation_count, 0, // decision_count filled later
        archive_days, total_signals,
        None, // translation_coverage — filled in publishing::agent_publish
    )
}
