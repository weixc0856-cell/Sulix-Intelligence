//! Domain types for the Daily Intelligence Brief generator.

/// A signal passed to the LLM for analysis. Derived from `signals_today()`.
pub struct SignalCandidate {
    pub id: String,
    pub title: String,
    pub category: String,
    pub signal_summary: String,
    pub article_count: usize,
    pub avg_score: f64,
    pub trend: String,
    /// Article IDs from this signal's evidence (used for evidence binding).
    pub article_ids: Vec<i64>,
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
