//! Synthesis — 综合摘要
//!
//! 从 clusterer.rs 拆分。

use crate::clusterer::{Theme, ThemeAnalysis};
use crate::domain::theme::Summary;

/// 整合所有主题分析，输出综合判断
pub fn synthesize(themes: &[Theme], analyses: &[ThemeAnalysis]) -> Summary {
    let mut narrative = String::new();

    let mut all_connections: Vec<&str> = Vec::new();
    for a in analyses {
        for c in &a.connections {
            if !all_connections.contains(&c.as_str()) {
                all_connections.push(c);
            }
        }
    }

    if analyses.len() >= 2 {
        narrative.push_str("多个主题指向同一方向：");
        if let Some(first) = analyses.first() {
            narrative.push_str(&format!("\n- {} → {}", first.theme_title, first.bluf));
        }
        for a in analyses.iter().skip(1) {
            narrative.push_str(&format!("\n- {} → {}", a.theme_title, a.bluf));
        }
    } else if let Some(first) = analyses.first() {
        narrative = first.bluf.clone();
    }

    Summary {
        headline: if analyses.len() >= 2 {
            format!("{} 个主题指向同一趋势", analyses.len())
        } else {
            "单主题深度分析".into()
        },
        narrative,
        total_articles: themes.iter().map(|t| t.articles.len()).sum(),
        theme_count: themes.len(),
    }
}
