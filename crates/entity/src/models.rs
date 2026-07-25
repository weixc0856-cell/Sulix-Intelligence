//! Core domain types for the Entity Graph.

use serde::{Deserialize, Serialize};

/// Entity relation type — defined as an enum so additions are explicit
/// and the type system prevents typos in `relation_type` column values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityRelationType {
    /// Co-occurrence in the same article (auto-generated from pipeline).
    MentionedTogether,
    /// One entity competes with another.
    CompetesWith,
    /// One entity depends on another (e.g. NVIDIA depends_on TSMC).
    DependsOn,
    /// One entity was acquired by another.
    AcquiredBy,
    /// One entity is a part of another (e.g. a subsidiary).
    PartOf,
}

impl EntityRelationType {
    /// Serialize to the string value stored in D1.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MentionedTogether => "mentioned_together",
            Self::CompetesWith => "competes_with",
            Self::DependsOn => "depends_on",
            Self::AcquiredBy => "acquired_by",
            Self::PartOf => "part_of",
        }
    }
}

// ---------------------------------------------------------------------------
// Response / query types
// ---------------------------------------------------------------------------

/// A single entity reference within an article context.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityRef {
    pub id: i64,
    pub name: String,
    pub normalized_name: String,
    pub entity_type: String,
    pub relevance: f64,
    pub context: Option<String>,
}

/// Summary row for entity-listing endpoints.
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

/// Full entity detail, including aggregate article count.
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

/// A related entity connected through `entity_relations`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RelatedEntity {
    pub id: i64,
    pub name: String,
    pub entity_type: String,
    pub relation_type: String,
    pub confidence: f64,
    pub last_seen_at: i64,
}
