//! 趋势分析：将 Trend Layer 数据写入 MemoryEngine

use crate::db::TrendRow;
use crate::domain::evidence::{Evidence, Stance};
use crate::engine::memory::MemoryEngine;

/// 趋势检测：将 Trend Layer 数据写入 MemoryEngine
///
/// change_pct > 30% 的趋势匹配到对应 Thesis 后追加 Evidence。
pub fn analyze_trends(trends: &[TrendRow], memory: &mut MemoryEngine, today: &str) {
    for t in trends {
        if t.change_pct.abs() < 30.0 {
            continue;
        }
        let arrow = if t.change_pct > 0.0 { "↑" } else { "↓" };
        let stance = if t.change_pct > 0.0 {
            Stance::Supports
        } else {
            Stance::Challenges
        };
        let signal = ((t.change_pct.abs() / 10.0).round() as u8).min(10);

        if let Some(thesis) = memory.find_by_title_mut(&t.category) {
            thesis.evidences.push(Evidence {
                date: today.to_string(),
                title: format!("Trend: {} {} {:.0}%", t.category, arrow, t.change_pct.abs()),
                source: "Hermes.Trend".into(),
                summary: format!(
                    "{}: {} 篇 vs 前 {} 篇",
                    t.category, t.recent_count, t.prev_count
                ),
                stance,
                signal_strength: signal.max(4),
            });
            thesis.updated = today.to_string();
        }
    }
}
