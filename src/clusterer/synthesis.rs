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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clusterer::Theme;
    use crate::domain::theme::{ThemeAnalysis, Summary};
    use crate::fetcher::Article;

    fn make_theme(title: &str, article_count: usize) -> Theme {
        Theme {
            id: format!("t-{}", title),
            title: title.to_string(),
            summary: "test summary".into(),
            articles: vec![Article {
                id: String::new(), title: "test".into(), source: "test".into(), url: "".into(),
                content: None, summary: None, published_at: None,
                category: String::new(), wiki_summary: None,
                evidence_type: String::new(), is_internal: false,
            }; article_count],
            sources: vec!["test".into()],
        }
    }

    fn make_analysis(title: &str, bluf: &str, connections: Vec<String>) -> ThemeAnalysis {
        ThemeAnalysis {
            theme_id: format!("t-{}", title),
            theme_title: title.to_string(),
            bluf: bluf.to_string(),
            impact: String::new(),
            geopolitical_fact: String::new(),
            supply_chain_impact: String::new(),
            analysis_paragraph: String::new(),
            evidence_level: String::new(),
            signal_strength: 5,
            fact_base: vec![],
            connections,
            source_urls: vec![],
            assumptions: vec![],
            adverse: None,
            next_tests: vec![],
            open_questions: vec![],
            chains: vec![],
            what_to_do: String::new(),
            what_to_watch: String::new(),
        }
    }

    #[test]
    fn test_synthesize_single_theme() {
        let themes = vec![make_theme("AI Commoditization", 3)];
        let analyses = vec![make_analysis("AI Commoditization", "模型商品化加速", vec![])];
        let summary = synthesize(&themes, &analyses);
        assert_eq!(summary.headline, "单主题深度分析");
        assert!(summary.narrative.contains("模型商品化加速"));
        assert_eq!(summary.total_articles, 3);
        assert_eq!(summary.theme_count, 1);
    }

    #[test]
    fn test_synthesize_multiple_themes() {
        let themes = vec![
            make_theme("AI Commoditization", 2),
            make_theme("Agent Market", 3),
        ];
        let analyses = vec![
            make_analysis("AI Commoditization", "模型价格下降", vec![]),
            make_analysis("Agent Market", "Agent 采用率上升", vec!["AI Commoditization".into()]),
        ];
        let summary = synthesize(&themes, &analyses);
        assert!(summary.headline.contains("2 个主题"));
        assert!(summary.narrative.contains("AI Commoditization"));
        assert!(summary.narrative.contains("Agent Market"));
        assert_eq!(summary.total_articles, 5);
        assert_eq!(summary.theme_count, 2);
    }

    #[test]
    fn test_synthesize_empty() {
        let summary = synthesize(&[], &[]);
        assert_eq!(summary.total_articles, 0);
        assert_eq!(summary.theme_count, 0);
    }

    #[test]
    fn test_synthesize_connections_dedup() {
        let themes = vec![make_theme("Theme A", 1), make_theme("Theme B", 1)];
        let analyses = vec![
            make_analysis("Theme A", "A", vec!["Connection X".into(), "Connection Y".into()]),
            make_analysis("Theme B", "B", vec!["Connection X".into()]),
        ];
        let summary = synthesize(&themes, &analyses);
        assert!(summary.headline.contains("2 个主题"));
        assert!(summary.narrative.contains("Theme A"));
        assert!(summary.narrative.contains("Theme B"));
    }
}
