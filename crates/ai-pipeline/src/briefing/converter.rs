//! Conversions from store types to briefing domain types.

use crate::briefing::types::{EvidenceArticle, SignalCandidate};

impl From<store::SignalBriefInput> for SignalCandidate {
    fn from(input: store::SignalBriefInput) -> Self {
        Self {
            id: format!("thread:{}", input.thread_id),
            title: input.title,
            category: String::new(),
            signal_summary: input.description,
            article_count: input.recent_article_count as usize,
            source_count: input.source_count as usize,
            avg_score: input.current_score,
            trend: input.trend,
            articles: input.evidence.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<store::BriefArticle> for EvidenceArticle {
    fn from(input: store::BriefArticle) -> Self {
        Self {
            id: input.id,
            title: input.title,
            url: None,
            feed_name: None,
            score: input.score,
        }
    }
}
