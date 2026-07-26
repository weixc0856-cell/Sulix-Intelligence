use serde::{Deserialize, Serialize};

// ── Intent ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub intent_type: String, // decision_support | reflection | pattern_analysis
    pub stage: CognitiveStage,
    pub desired_outcome: DesiredOutcome,
    pub domain: Option<String>,
    pub action: Option<String>,
    pub entity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CognitiveStage {
    Explore,
    Decide,
    Review,
    Learn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DesiredOutcome {
    Recommendation,
    Explanation,
    Comparison,
    Prediction,
}

// ── Retrieval Plan ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalPlan {
    pub decision_query: Option<DecisionQuery>,
    pub reflection_query: Option<ReflectionQuery>,
    pub memory_query: Option<MemoryQuery>,
    pub pattern_enabled: bool,
    pub max_results: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionQuery {
    pub domain: Option<String>,
    pub status: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionQuery {
    pub status: Option<String>,
    pub min_quality: Option<f64>,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuery {
    pub memory_types: Vec<String>,
    pub status: Option<String>,
    pub min_confidence: Option<f64>,
    pub limit: u32,
}

// ── Context Items (unified) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextItem {
    Decision(ScoredDecision),
    Reflection(ScoredReflection),
    Memory(ScoredMemory),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoredDecision {
    pub id: String,
    pub title: String,
    pub decision_type: String,
    pub status: String,
    pub confidence: f64,
    pub relevance_score: f64,
    pub rank_components: RankComponents,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoredReflection {
    pub id: String,
    pub result: Option<String>,
    pub quality_score: f64,
    pub relevance_score: f64,
    pub rank_components: RankComponents,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoredMemory {
    pub id: String,
    pub statement: String,
    pub memory_type: String,
    pub confidence: f64,
    pub relevance_score: f64,
    pub rank_components: RankComponents,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RankComponents {
    pub query_alignment: f64,
    pub confidence: f64,
    pub recency: f64,
    pub usage_frequency: f64,
    pub user_specificity: f64,
}

// ── Context Evidence (lineage) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEvidence {
    pub source_type: String,
    pub source_id: String,
    pub selection_reason: String,
    pub relevance_score: f64,
}

// ── Pattern ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternContext {
    pub pattern_type: String,
    pub description: String,
    pub frequency: u32,
    pub evidence_refs: Vec<String>,
}

// ── Confidence ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfidence {
    pub overall: f64,
    pub coverage: f64,
    pub data_quality: f64,
    pub recency: f64,
    pub consistency: f64,
}

// ── Agent Context (final output) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub snapshot_id: String,
    pub query: String,
    pub intent: Intent,
    pub evidence: Vec<ContextEvidence>,
    pub decisions: Vec<ScoredDecision>,
    pub reflections: Vec<ScoredReflection>,
    pub memories: Vec<ScoredMemory>,
    pub patterns: Vec<PatternContext>,
    pub confidence: ContextConfidence,
    pub engine_version: String,
    pub generated_at: i64,
}

// ── Internal API types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRequest {
    pub query: String,
    pub options: Option<ContextRequestOptions>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextRequestOptions {
    pub include_patterns: Option<bool>,
    pub max_results: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResponse {
    pub snapshot_id: String,
    pub context: AgentContext,
}
