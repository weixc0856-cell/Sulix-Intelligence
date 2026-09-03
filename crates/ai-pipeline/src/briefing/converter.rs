//! Conversions from owned briefing input DTOs to briefing domain types.
//!
//! The input DTOs ([`BriefSignalInput`]) are ai-pipeline-owned; the composition
//! root maps store rows onto them before calling into the briefing generator.
//! No store type appears in this crate.

use crate::briefing::context::{BriefingContext, EntityContext};
use crate::briefing::types::{
    BriefArticleInput, BriefSignalInput, EvidenceArticle, RelatedEntityInput, SignalCandidate,
};

impl From<BriefSignalInput> for SignalCandidate {
    fn from(input: BriefSignalInput) -> Self {
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
            context: BriefingContext {
                entities: input.related_entities.into_iter().map(Into::into).collect(),
                decisions: Vec::new(), // populated by briefing job
            },
        }
    }
}

impl From<BriefArticleInput> for EvidenceArticle {
    fn from(input: BriefArticleInput) -> Self {
        Self { id: input.id, title: input.title, url: input.url, feed_name: input.feed_name, score: input.score }
    }
}

impl From<RelatedEntityInput> for EntityContext {
    fn from(input: RelatedEntityInput) -> Self {
        Self { name: input.name, entity_type: input.entity_type, relevance: input.confidence.unwrap_or(0.5) }
    }
}
