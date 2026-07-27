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
