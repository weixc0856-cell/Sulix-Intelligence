//! Domain types for the Daily Intelligence Brief generator.

use super::context::BriefingContext;

/// A signal passed to the LLM for analysis.
///
/// Carries both signal metrics and a `context` field with entity, decision,
/// and evaluation context so the LLM can produce decision-aware analysis.
pub struct SignalCandidate {
    pub id: String,
    pub title: String,
    pub category: String,
    pub signal_summary: String,
    pub article_count: usize,
    pub source_count: usize,
    pub avg_score: f64,
    pub trend: String,
    /// Evidence articles from this signal, carrying title/url/score for evidence binding.
    pub articles: Vec<EvidenceArticle>,
    /// Intelligence context — entities, decisions, evaluations.
    pub context: BriefingContext,
}

/// Single insight returned by the LLM with evidence signal references.
#[derive(Debug, serde::Deserialize)]
pub struct GeneratedInsight {
    pub title: String,
    pub category: String,
    pub summary: String,
    pub why_it_matters: String,
    pub recommendation: String,
    #[serde(deserialize_with = "deserialize_impact")]
    pub impact: String,
    pub confidence: f64,
    #[serde(default)]
    pub evidence_signal_ids: Vec<String>,
}

fn deserialize_impact<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let s: String = serde::Deserialize::deserialize(d)?;
    match s.as_str() {
        "High" | "Medium" | "Low" => Ok(s),
        _ => Err(serde::de::Error::custom(format!("invalid impact: {s}"))),
    }
}

/// Top-level LLM output shape.
#[derive(Debug, serde::Deserialize)]
pub struct LlmOutput {
    #[serde(default)]
    pub schema_version: u32,
    pub insights: Vec<GeneratedInsight>,
}

// ---------------------------------------------------------------------------
// Output types — serialised to JSON and stored in D1
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Briefing {
    pub date: String,
    pub generated_at: i64,
    pub signal_count: u32,
    pub insights: Vec<Insight>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Insight {
    pub title: String,
    pub category: String,
    pub summary: String,
    pub why_it_matters: String,
    pub recommendation: String,
    pub impact: String,
    pub confidence: f64,
    pub evidence_count: u32,
    pub source_count: u32,
    pub trend: String,
    pub articles: Vec<EvidenceArticle>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EvidenceArticle {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub feed_name: Option<String>,
    pub score: f64,
}

// ---------------------------------------------------------------------------
// Input DTOs — owned by ai-pipeline so the briefing generator never sees
// store types. The composition root (worker-entry) maps store rows onto these
// before calling into briefing; converter.rs projects them onto candidates.
// ---------------------------------------------------------------------------

/// Projection of a signal thread fed into the briefing generator.
#[derive(Debug, Clone)]
pub struct BriefSignalInput {
    pub thread_id: i64,
    pub title: String,
    pub description: String,
    pub recent_article_count: i64,
    pub source_count: i64,
    pub current_score: f64,
    pub trend: String,
    pub evidence: Vec<BriefArticleInput>,
    pub related_entities: Vec<RelatedEntityInput>,
}

#[derive(Debug, Clone)]
pub struct BriefArticleInput {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub feed_name: Option<String>,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct RelatedEntityInput {
    pub name: String,
    pub entity_type: String,
    pub confidence: Option<f64>,
}
