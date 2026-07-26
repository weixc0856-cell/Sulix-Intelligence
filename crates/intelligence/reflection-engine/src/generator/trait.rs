//! ReflectionGenerator trait — abstraction over LLM providers.
//!
//! Not bound to HttpSummarizer.  Future: DeepSeek, OpenRouter, Cloudflare AI, Local.

use async_trait::async_trait;

use crate::context::ReflectionContext;

/// A draft reflection — the raw LLM output before validation and persistence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReflectionDraft {
    pub result: String,
    pub confidence_calibration: String,
    pub quality_score: f64,
    pub lessons: Vec<LessonDraft>,
    pub rules: Vec<RuleDraft>,
}

/// A single lesson extracted from the decision-outcome pair.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LessonDraft {
    pub category: String,
    pub domain: String,
    pub description: String,
    pub severity: String,
    pub confidence: f64,
    pub evidence_basis: Vec<String>,
}

/// A decision rule derived from the reflection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuleDraft {
    pub condition_domain: String,
    pub condition_trigger: String,
    pub action_type: String,
    pub action_instruction: String,
    pub confidence: f64,
}

/// Generates a ReflectionDraft from context.
#[async_trait(?Send)]
pub trait ReflectionGenerator {
    async fn generate(&self, context: &ReflectionContext) -> Result<ReflectionDraft, String>;
}
