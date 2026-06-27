//! 冲突应用 + Thesis 发现

use crate::archive::ChronicleDb;
use crate::domain::theme::ThemeAnalysis;
use crate::domain::evidence::{Evidence, Stance};
use crate::engine::memory::MemoryEngine;

use super::ChangeSummary;

/// 矛盾写入：将 ChangeSummary 中的冲突记到对应 Thesis 的 Challenges Evidence
pub fn apply_conflicts(changes: &ChangeSummary, memory: &mut MemoryEngine, today: &str) {
    for conflict in &changes.conflicts {
        if let Some(thesis) = memory.find_by_title_mut(&conflict.topic) {
            thesis.evidences.push(Evidence {
                date: today.to_string(),
                title: conflict.today_signal.clone(),
                source: "Hermes.Conflict".into(),
                summary: conflict.prior_belief.clone(),
                stance: Stance::Challenges,
                signal_strength: 8,
            });
            thesis.updated = today.to_string();
        }
    }
}

/// 新 Thesis 发现：根据历史 Chronicle 自动创建 Thesis
///
/// 如果某主题关键词在过去 7 天的 chronicle 中出现 >= 2 次
/// 且 MemoryEngine 中尚不存在 → 自动创建新 Thesis。
pub fn discover_theses(
    analyses: &[ThemeAnalysis],
    chronicle: &ChronicleDb,
    memory: &mut MemoryEngine,
    today: &str,
) {
    use chrono::NaiveDate;

    let recent_topics: Vec<&str> = chronicle
        .entries
        .iter()
        .filter(|e| {
            if let Ok(d) = NaiveDate::parse_from_str(&e.date, "%Y-%m-%d") {
                if let Ok(t) = NaiveDate::parse_from_str(today, "%Y-%m-%d") {
                    let diff = (t - d).num_days();
                    return (0..=7).contains(&diff);
                }
            }
            false
        })
        .map(|e| e.topic.as_str())
        .collect();

    for analysis in analyses {
        if memory.find_by_title(&analysis.theme_title).is_some() {
            continue;
        }
        let count = recent_topics
            .iter()
            .filter(|t| {
                let words: Vec<&str> = analysis
                    .theme_title
                    .split_whitespace()
                    .filter(|w| w.len() > 1)
                    .collect();
                words.iter().any(|w| t.contains(w))
            })
            .count();
        if count >= 2 {
            memory.force_thesis(analysis.theme_title.clone(), today, &analysis.bluf);
            log::info!("🧪 Hermes 发现新 Thesis: {}", analysis.theme_title);
        }
    }
}
