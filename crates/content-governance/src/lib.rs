//! Content Governance — policy evaluation engine for content compliance.
//!
//! Determines whether content from a given source may be:
//! - Stored (full-text in R2)
//! - Served (via API)
//! - Embedded (vectorized for similarity search)
//! - AI-summarized
//!
//! This crate is **pure logic** with no Cloudflare Worker dependencies,
//! making it fully testable in standard Rust unit tests.

use store::Source;

// ── Permission enums (nested, not flat booleans) ──

/// Permission for storing full-text content in R2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoragePermission {
    Denied,
    Allowed,
}

/// Permission for serving raw content via API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServingPermission {
    Denied,
    Allowed,
}

/// Permission for embedding content in vector search indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingPermission {
    Denied,
    /// Only embed metadata (title, tags, summary), not full text.
    Limited,
    Allowed,
}

/// Permission for AI summarisation of content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiSummaryPermission {
    /// Only use the source title for AI context, no body.
    TitleOnly,
    Allowed,
}

/// Result of evaluating a source's content policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    pub storage: StoragePermission,
    pub serving: ServingPermission,
    pub embedding: EmbeddingPermission,
    pub ai_summary: AiSummaryPermission,
    pub retention_days: u32,
    pub attribution_required: bool,
    pub reason: String,
}

impl Default for PolicyDecision {
    fn default() -> Self {
        Self {
            storage: StoragePermission::Denied,
            serving: ServingPermission::Denied,
            embedding: EmbeddingPermission::Denied,
            ai_summary: AiSummaryPermission::TitleOnly,
            retention_days: 7,
            attribution_required: true,
            reason: "default: no policy set".into(),
        }
    }
}

/// Evaluate a source's content policy and produce a `PolicyDecision`.
///
/// Policy matrix:
///
/// | Policy          | storage | serving | embedding | AI summary | retention |
/// |-----------------|---------|---------|-----------|------------|-----------|
/// | MetadataOnly    | Denied  | Denied  | Denied    | TitleOnly  | 7d        |
/// | SummaryAllowed  | Denied  | Denied  | Limited   | Allowed    | 30d       |
/// | FullTextAllowed | Allowed | Allowed | Allowed   | Allowed    | 30d       |
/// | UserOwned       | Allowed | Allowed | Allowed   | Allowed    | 90d       |
pub fn evaluate_policy(source: &Source) -> PolicyDecision {
    match source.policy.as_str() {
        "MetadataOnly" => PolicyDecision {
            storage: StoragePermission::Denied,
            serving: ServingPermission::Denied,
            embedding: EmbeddingPermission::Denied,
            ai_summary: AiSummaryPermission::TitleOnly,
            retention_days: source.retention_days.map(|d| d as u32).unwrap_or(7),
            attribution_required: true,
            reason: "Source policy: metadata only".into(),
        },
        "SummaryAllowed" => PolicyDecision {
            storage: StoragePermission::Denied,
            serving: ServingPermission::Denied,
            embedding: EmbeddingPermission::Limited,
            ai_summary: AiSummaryPermission::Allowed,
            retention_days: source.retention_days.map(|d| d as u32).unwrap_or(30),
            attribution_required: true,
            reason: "Source policy: summary allowed, no full-text".into(),
        },
        "FullTextAllowed" => PolicyDecision {
            storage: StoragePermission::Allowed,
            serving: ServingPermission::Allowed,
            embedding: EmbeddingPermission::Allowed,
            ai_summary: AiSummaryPermission::Allowed,
            retention_days: source.retention_days.map(|d| d as u32).unwrap_or(30),
            attribution_required: source.tier != "Tier0",
            reason: "Source policy: full text allowed".into(),
        },
        "UserOwned" => PolicyDecision {
            storage: StoragePermission::Allowed,
            serving: ServingPermission::Allowed,
            embedding: EmbeddingPermission::Allowed,
            ai_summary: AiSummaryPermission::Allowed,
            retention_days: source.retention_days.map(|d| d as u32).unwrap_or(90),
            attribution_required: true,
            reason: "User-owned content".into(),
        },
        other => PolicyDecision {
            storage: StoragePermission::Denied,
            serving: ServingPermission::Denied,
            embedding: EmbeddingPermission::Denied,
            ai_summary: AiSummaryPermission::TitleOnly,
            retention_days: 7,
            attribution_required: true,
            reason: format!("Unknown policy: {other}"),
        },
    }
}

/// Shorthand: does the source policy allow full-text extraction and storage?
pub fn can_extract(source: &Source) -> bool {
    matches!(source.policy.as_str(), "FullTextAllowed" | "UserOwned")
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::Source;

    fn make_source(policy: &str) -> Source {
        Source {
            id: 1,
            source_type: "RssFeed".into(),
            feed_id: Some(1),
            name: Some("test".into()),
            tier: "Tier2".into(),
            policy: policy.into(),
            license: "Unknown".into(),
            license_detail: None,
            attribution: Some("Test Source".into()),
            trust_score: None,
            retention_days: None,
            verified: false,
            notes: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn metadata_only_denies_everything() {
        let source = make_source("MetadataOnly");
        let d = evaluate_policy(&source);
        assert_eq!(d.storage, StoragePermission::Denied);
        assert_eq!(d.serving, ServingPermission::Denied);
        assert_eq!(d.embedding, EmbeddingPermission::Denied);
        assert_eq!(d.ai_summary, AiSummaryPermission::TitleOnly);
        assert_eq!(d.retention_days, 7);
        assert!(d.attribution_required);
    }

    #[test]
    fn summary_allows_ai_and_limited_embedding() {
        let source = make_source("SummaryAllowed");
        let d = evaluate_policy(&source);
        assert_eq!(d.storage, StoragePermission::Denied);
        assert_eq!(d.serving, ServingPermission::Denied);
        assert_eq!(d.embedding, EmbeddingPermission::Limited);
        assert_eq!(d.ai_summary, AiSummaryPermission::Allowed);
        assert_eq!(d.retention_days, 30);
    }

    #[test]
    fn full_text_allows_with_attribution() {
        let source = make_source("FullTextAllowed");
        let d = evaluate_policy(&source);
        assert_eq!(d.storage, StoragePermission::Allowed);
        assert_eq!(d.serving, ServingPermission::Allowed);
        assert_eq!(d.embedding, EmbeddingPermission::Allowed);
        assert_eq!(d.ai_summary, AiSummaryPermission::Allowed);
        assert!(d.attribution_required);
    }

    #[test]
    fn full_text_tier0_no_attribution() {
        let mut source = make_source("FullTextAllowed");
        source.tier = "Tier0".into();
        let d = evaluate_policy(&source);
        assert_eq!(d.storage, StoragePermission::Allowed);
        assert!(!d.attribution_required);
    }

    #[test]
    fn user_owned_long_retention() {
        let source = make_source("UserOwned");
        let d = evaluate_policy(&source);
        assert_eq!(d.storage, StoragePermission::Allowed);
        assert_eq!(d.serving, ServingPermission::Allowed);
        assert_eq!(d.retention_days, 90);
    }

    #[test]
    fn custom_retention_overrides_default() {
        let mut source = make_source("FullTextAllowed");
        source.retention_days = Some(14);
        let d = evaluate_policy(&source);
        assert_eq!(d.retention_days, 14);
    }

    #[test]
    fn can_extract_full_text() {
        assert!(can_extract(&make_source("FullTextAllowed")));
        assert!(can_extract(&make_source("UserOwned")));
        assert!(!can_extract(&make_source("SummaryAllowed")));
        assert!(!can_extract(&make_source("MetadataOnly")));
    }

    #[test]
    fn unknown_policy_defaults_to_deny() {
        let source = make_source("BogusPolicy");
        let d = evaluate_policy(&source);
        assert_eq!(d.storage, StoragePermission::Denied);
        assert_eq!(d.serving, ServingPermission::Denied);
        assert_eq!(d.retention_days, 7);
    }
}
