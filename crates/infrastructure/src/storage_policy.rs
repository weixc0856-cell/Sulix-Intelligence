//! Storage Policy — connects content-governance `PolicyDecision` to artifact lifecycle.
//!
//! This module enforces retention and access policies when storing or retrieving
//! artifacts. The GC job uses `should_retain()` to determine which artifact types
//! can be pruned based on source tier and policy.

use content_governance::PolicyDecision;

/// Whether an artifact type is subject to the source's retention policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactCategory {
    /// Raw article content — governed by source policy.
    ArticleContent,
    /// AI-generated summary — governed by source policy.
    AiSummary,
    /// AI-generated claim analysis — governed by source policy.
    ClaimAnalysis,
    /// Decision memo — user-owned, long retention.
    DecisionMemo,
    /// Reflection result — derived from outcomes, medium retention.
    ReflectionResult,
    /// Reasoning trace — debugging, short retention.
    ReasoningTrace,
}

impl ArtifactCategory {
    pub fn from_type(s: &str) -> Self {
        match s {
            "article_content" => Self::ArticleContent,
            "ai_summary" => Self::AiSummary,
            "claim_analysis" => Self::ClaimAnalysis,
            "decision_memo" => Self::DecisionMemo,
            "reflection_result" => Self::ReflectionResult,
            "reasoning_trace" => Self::ReasoningTrace,
            _ => Self::ReasoningTrace, // default: short retention for unknown types
        }
    }
}

/// Determine the retention duration in days for an artifact category.
///
/// Artifacts older than this may be pruned by the GC job.
pub fn retention_days(category: &ArtifactCategory, source_policy: Option<&PolicyDecision>) -> u32 {
    match category {
        // Full-text content is governed by source policy
        ArtifactCategory::ArticleContent => source_policy.map(|p| p.retention_days).unwrap_or(7),
        // AI summaries follow source policy
        ArtifactCategory::AiSummary => source_policy.map(|p| p.retention_days).unwrap_or(7),
        // Claim analysis follows source policy
        ArtifactCategory::ClaimAnalysis => source_policy.map(|p| p.retention_days).unwrap_or(7),
        // Decision memos retain long-term (unless source says otherwise)
        ArtifactCategory::DecisionMemo => source_policy.map(|p| p.retention_days.max(90)).unwrap_or(365),
        // Reflection results are kept for medium duration
        ArtifactCategory::ReflectionResult => 90,
        // Raw reasoning traces are debugging-only — short retention
        ArtifactCategory::ReasoningTrace => 30,
    }
}

/// Check whether an artifact of the given type should be retained.
///
/// Sources with `StoragePermission::Denied` should have no stored artifacts
/// at all (metadata-only sources). This is enforced at write time.
pub fn can_store(category: &ArtifactCategory, source_policy: Option<&PolicyDecision>) -> bool {
    match category {
        ArtifactCategory::ArticleContent | ArtifactCategory::AiSummary => {
            source_policy.map(|p| p.storage == content_governance::StoragePermission::Allowed).unwrap_or(false)
        }
        // Decision and reflection artifacts are always permitted (user-generated)
        ArtifactCategory::DecisionMemo | ArtifactCategory::ReflectionResult => true,
        // Claim analysis depends on source permission
        ArtifactCategory::ClaimAnalysis => {
            source_policy.map(|p| p.storage == content_governance::StoragePermission::Allowed).unwrap_or(false)
        }
        // Reasoning traces are always permitted (internal debugging)
        ArtifactCategory::ReasoningTrace => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use content_governance::{PolicyDecision, StoragePermission};

    fn policy(storage: StoragePermission, retention_days: u32) -> PolicyDecision {
        PolicyDecision { storage, retention_days, ..PolicyDecision::default() }
    }

    #[test]
    fn from_type_maps_known_types_and_defaults_unknown() {
        assert_eq!(ArtifactCategory::from_type("article_content"), ArtifactCategory::ArticleContent);
        assert_eq!(ArtifactCategory::from_type("ai_summary"), ArtifactCategory::AiSummary);
        assert_eq!(ArtifactCategory::from_type("claim_analysis"), ArtifactCategory::ClaimAnalysis);
        assert_eq!(ArtifactCategory::from_type("decision_memo"), ArtifactCategory::DecisionMemo);
        assert_eq!(ArtifactCategory::from_type("reflection_result"), ArtifactCategory::ReflectionResult);
        assert_eq!(ArtifactCategory::from_type("reasoning_trace"), ArtifactCategory::ReasoningTrace);
        // Unknown types degrade to short-retention default.
        assert_eq!(ArtifactCategory::from_type("mystery_type"), ArtifactCategory::ReasoningTrace);
    }

    #[test]
    fn retention_days_without_policy_uses_category_defaults() {
        assert_eq!(retention_days(&ArtifactCategory::ArticleContent, None), 7);
        assert_eq!(retention_days(&ArtifactCategory::AiSummary, None), 7);
        assert_eq!(retention_days(&ArtifactCategory::ClaimAnalysis, None), 7);
        assert_eq!(retention_days(&ArtifactCategory::DecisionMemo, None), 365);
        assert_eq!(retention_days(&ArtifactCategory::ReflectionResult, None), 90);
        assert_eq!(retention_days(&ArtifactCategory::ReasoningTrace, None), 30);
    }

    #[test]
    fn retention_days_follows_source_policy_where_governed() {
        let src = policy(StoragePermission::Allowed, 14);
        assert_eq!(retention_days(&ArtifactCategory::ArticleContent, Some(&src)), 14);
        assert_eq!(retention_days(&ArtifactCategory::AiSummary, Some(&src)), 14);
        assert_eq!(retention_days(&ArtifactCategory::ClaimAnalysis, Some(&src)), 14);
    }

    #[test]
    fn decision_memo_respects_policy_but_never_below_ninety() {
        assert_eq!(retention_days(&ArtifactCategory::DecisionMemo, Some(&policy(StoragePermission::Allowed, 14))), 90);
        assert_eq!(
            retention_days(&ArtifactCategory::DecisionMemo, Some(&policy(StoragePermission::Allowed, 400))),
            400
        );
    }

    #[test]
    fn derived_categories_ignore_policy() {
        let src = policy(StoragePermission::Allowed, 14);
        assert_eq!(retention_days(&ArtifactCategory::ReflectionResult, Some(&src)), 90);
        assert_eq!(retention_days(&ArtifactCategory::ReasoningTrace, Some(&src)), 30);
    }

    #[test]
    fn can_store_respects_source_permission_for_source_governed_types() {
        assert!(can_store(&ArtifactCategory::ArticleContent, Some(&policy(StoragePermission::Allowed, 7))));
        assert!(!can_store(&ArtifactCategory::ArticleContent, Some(&policy(StoragePermission::Denied, 7))));
        assert!(!can_store(&ArtifactCategory::ArticleContent, None));
        assert!(can_store(&ArtifactCategory::AiSummary, Some(&policy(StoragePermission::Allowed, 7))));
        assert!(!can_store(&ArtifactCategory::AiSummary, None));
        assert!(can_store(&ArtifactCategory::ClaimAnalysis, Some(&policy(StoragePermission::Allowed, 7))));
        assert!(!can_store(&ArtifactCategory::ClaimAnalysis, None));
    }

    #[test]
    fn can_store_always_true_for_user_generated_and_debug_artifacts() {
        assert!(can_store(&ArtifactCategory::DecisionMemo, None));
        assert!(can_store(&ArtifactCategory::ReflectionResult, None));
        assert!(can_store(&ArtifactCategory::ReasoningTrace, None));
        assert!(can_store(&ArtifactCategory::DecisionMemo, Some(&policy(StoragePermission::Denied, 7))));
        assert!(can_store(&ArtifactCategory::ReasoningTrace, Some(&policy(StoragePermission::Denied, 7))));
    }
}
