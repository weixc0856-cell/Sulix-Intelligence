//! Source registry — content provenance and governance metadata for every
//! information source (RSS feeds, API endpoints, user uploads, etc.).

use serde::{Deserialize, Serialize};

/// Type of information source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceType {
    RssFeed,
    Api,
    Manual,
    UserUpload,
}

/// Content tier classification.
/// Determines trust weight and compliance handling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentTier {
    /// Open-source / public domain (arXiv, government publications, RFCs)
    Tier0,
    /// Creative Commons or permissive license
    Tier1,
    /// News media / commercial (fair use considerations)
    Tier2,
    /// User-private content (personal blogs, internal tools)
    Tier3,
}

/// Content usage policy — what the system may do with content from this source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentPolicy {
    /// Metadata only (title, URL, excerpt). No full-text, no AI.
    MetadataOnly,
    /// Summarisation allowed, but not full-text storage or embedding.
    SummaryAllowed,
    /// Full-text may be stored and served. AI analysis permitted.
    FullTextAllowed,
    /// User-owned content (full rights).
    UserOwned,
}

/// License type for a source's content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseType {
    PublicDomain,
    CreativeCommons,
    Permissive,
    Copyright,
    FairUse,
    Other,
    Unknown,
}

/// A registered content source with governance metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: i64,
    pub source_type: String,  // serialized SourceType
    pub feed_id: Option<i64>, // FK to feeds (nullable for non-feed sources)
    pub name: Option<String>,
    pub tier: String,    // serialized ContentTier
    pub policy: String,  // serialized ContentPolicy
    pub license: String, // serialized LicenseType
    pub license_detail: Option<String>,
    pub attribution: Option<String>,
    pub trust_score: Option<f64>,
    pub retention_days: Option<i64>,
    pub verified: bool,
    pub notes: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Input for creating or updating a Source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSource {
    pub source_type: String,
    pub feed_id: Option<i64>,
    pub name: Option<String>,
    pub tier: String,
    pub policy: String,
    pub license: String,
    pub license_detail: Option<String>,
    pub attribution: Option<String>,
    pub trust_score: Option<f64>,
    pub retention_days: Option<i64>,
    pub verified: bool,
    pub notes: Option<String>,
}
