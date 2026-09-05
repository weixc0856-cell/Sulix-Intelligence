use serde::{Deserialize, Serialize};

// ===== Entity Graph types =====

/// Summary row for entity listing.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntitySummary {
    pub id: i64,
    pub name: String,
    pub normalized_name: String,
    pub entity_type: String,
    pub canonical_id: Option<i64>,
    pub article_count: i64,
    pub last_seen: i64,
}

/// Full entity detail with aggregate counts.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityDetail {
    pub id: i64,
    pub name: String,
    pub normalized_name: String,
    pub entity_type: String,
    pub canonical_id: Option<i64>,
    pub description: Option<String>,
    pub metadata: Option<String>,
    pub article_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Entity reference within an article context.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityRef {
    pub id: i64,
    pub name: String,
    pub normalized_name: String,
    pub entity_type: String,
    pub relevance: f64,
    pub context: Option<String>,
}

/// Related entity through entity_relations edges.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RelatedEntity {
    pub id: i64,
    pub name: String,
    pub entity_type: String,
    pub relation_type: String,
    pub confidence: f64,
    pub last_seen_at: i64,
}

// ===== Entity Intelligence types =====

/// An article linked to an entity (Evidence).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityArticle {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub feed_name: Option<String>,
    pub published_at: Option<i64>,
    pub ai_summary: String,
    pub score: f64,
}

/// Activity summary for an entity over a time window.
/// Named "activity" not "signal" to reserve "signal" for Signal Engine V2.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityActivitySummary {
    pub article_count: i64,
    pub source_count: i64,
    pub avg_score: f64,
    pub max_score: f64,
    pub first_seen_at: Option<i64>,
    pub last_seen_at: Option<i64>,
    pub trend: String, // "rising", "stable", "declining"
}
