use serde::{Deserialize, Serialize};

/// How a signal thread was discovered — provenance tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    Entity,
    Semantic,
    Hybrid,
}

impl std::fmt::Display for DiscoveryMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Entity => write!(f, "entity"),
            Self::Semantic => write!(f, "semantic"),
            Self::Hybrid => write!(f, "hybrid"),
        }
    }
}

impl From<&str> for DiscoveryMethod {
    fn from(s: &str) -> Self {
        match s {
            "semantic" => Self::Semantic,
            "hybrid" => Self::Hybrid,
            _ => Self::Entity,
        }
    }
}

/// Provenance metadata for a signal — how and why it was discovered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalProvenance {
    pub method: DiscoveryMethod,
    pub score: Option<f64>,
}

impl Default for SignalProvenance {
    fn default() -> Self {
        Self { method: DiscoveryMethod::Entity, score: None }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalEvidence {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub feed_name: Option<String>,
    pub published_at: Option<i64>,
    pub score: f64,
}

/// Signal origin — which engine generated this signal.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SignalOrigin {
    #[default]
    Entity,
    LegacyScoreBucket,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TodaySignal {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub confidence: f64,
    pub evidence_count: i64,
    pub trend: String,
    pub articles: Vec<SignalEvidence>,
    /// Which engine generated this signal.
    #[serde(default)]
    pub origin: SignalOrigin,
    /// Entity anchor, if the signal was entity-derived.
    pub anchor_entity: Option<EntitySignalRef>,
}

// ===== Intelligence Signal types =====

/// Core Intelligence Signal — first-class artifact, NOT an entity ranking.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IntelligenceSignal {
    pub id: i64,
    pub anchor_entity_id: Option<i64>,
    pub title: String,
    pub summary: String,
    pub signal_type: String,
    pub confidence: f64,
    pub impact: String,
    pub trend: String,
    pub article_count: i64,
    pub source_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Transient candidate before materialization.
#[derive(Debug, Clone)]
pub struct EntitySignalCandidate {
    pub entity_id: i64,
    pub entity_name: String,
    pub entity_type: String,
    pub score: f64,
    pub volume: f64,
    pub diversity: f64,
    pub quality: f64,
    pub velocity: f64,
    pub novelty: f64,
    pub article_count: i64,
    pub source_count: i64,
    pub avg_score: f64,
    pub trend: String,
    pub evidence: Vec<SignalEvidence>,
    pub related_entity_ids: Vec<i64>,
}

/// Lightweight entity reference for API DTO.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntitySignalRef {
    pub id: i64,
    pub name: String,
    pub entity_type: String,
}

// ===== Signal Thread types =====

/// Signal Thread — long-lived intelligence asset.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalThread {
    pub id: i64,
    pub signal_key: String,
    pub anchor_entity_id: Option<i64>,
    pub title: String,
    pub description: String,
    pub status: String,
    pub health_score: f64,
    pub discovery_method: String,
    pub discovery_score: Option<f64>,
    pub first_seen_at: Option<i64>,
    pub last_seen_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Summary of a single signal instance for timeline display.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalInstanceSummary {
    pub id: i64,
    pub score: f64,
    pub confidence: f64,
    pub trend: String,
    pub article_count: i64,
    pub source_count: i64,
    pub generated_at: i64,
}

/// Briefing input — domain model assembled from signal threads.
/// Contains both current snapshot and cumulative metrics so the
/// LLM can distinguish "ongoing trend" from "spike event".
#[derive(Debug, Clone)]
pub struct SignalBriefInput {
    pub thread_id: i64,
    pub signal_key: String,
    pub anchor_entity: Option<String>,
    pub title: String,
    pub description: String,
    pub status: String,
    pub health_score: f64,
    /// Current score from the latest instance.
    pub current_score: f64,
    /// Current trend direction.
    pub trend: String,
    /// Total articles across all instances (thread lifetime).
    pub cumulative_article_count: i64,
    /// Articles in the last 7 days.
    pub recent_article_count: i64,
    /// Unique sources across recent instances.
    pub source_count: i64,
    /// Velocity ratio: recent / historical daily rate.
    pub velocity: f64,
    /// Recent instance timeline (for charting).
    pub instances: Vec<SignalInstanceSummary>,
    pub evidence: Vec<BriefArticle>,
    pub related_entities: Vec<RelatedEntityRef>,
    /// Provenance — how this signal was discovered.
    pub provenance: SignalProvenance,
}

/// Filter for listing signal threads.
#[derive(Debug, Clone)]
pub struct SignalThreadFilter {
    pub statuses: Vec<String>,
    pub limit: u32,
    pub min_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefArticle {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub feed_name: Option<String>,
    pub score: f64,
}

// ===== Radar / Projection types =====

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalHealthBreakdown {
    pub activity: f64,
    pub diversity: f64,
    pub quality: f64,
    pub velocity: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalHealth {
    pub score: f64,
    pub breakdown: SignalHealthBreakdown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalEvidenceSummary {
    pub articles: i64,
    pub sources: i64,
    pub avg_score: f64,
    pub last_seen: i64,
    pub velocity_24h: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalRadarItem {
    pub id: String,
    pub title: String,
    pub status: String,
    pub trend: String,
    pub health: SignalHealth,
    pub anchor_entity: Option<EntitySignalRef>,
    pub evidence: SignalEvidenceSummary,
    pub related: Vec<String>,
    pub first_seen_at: i64,
    pub last_evidence_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RadarDashboardSummary {
    pub total_active: i64,
    pub rising: i64,
    pub stable: i64,
    pub decaying: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RadarResponse {
    pub generated_at: i64,
    pub summary: RadarDashboardSummary,
    pub signals: Vec<SignalRadarItem>,
}

// ===== Signal Detail / Investigation types =====

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthComponents {
    pub volume: f64,
    pub diversity: f64,
    pub quality: f64,
    pub velocity: f64,
    pub persistence: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalHealthDetail2 {
    pub score: f64,
    pub components: HealthComponents,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalTimelineEvent {
    pub timestamp: i64,
    pub event_type: String,
    pub score: f64,
    pub article_count: i64,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RelatedSignalRef {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub health_score: f64,
}

/// Entity reference with relationship context for signal threads.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RelatedEntityRef {
    pub id: i64,
    pub name: String,
    pub entity_type: String,
    pub relation_type: String,
    /// Human-readable relationship label, e.g. "supplier", "competitor".
    pub relation: Option<String>,
    /// Confidence of the relationship link.
    pub confidence: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalDetail {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub trend: String,
    pub health: SignalHealthDetail2,
    pub anchor_entity: Option<EntitySignalRef>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub timeline: Vec<SignalTimelineEvent>,
    pub evidence_top: Vec<BriefArticle>,
    pub related_entities: Vec<RelatedEntityRef>,
    pub related_signals: Vec<RelatedSignalRef>,
    /// Rule-based "Why This Matters" analysis.
    pub analysis: Option<SignalAnalysis>,
}

/// Rule-based analysis for a signal thread — answers "Why This Matters".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalAnalysis {
    pub why_it_matters: String,
    pub impact: String,
    pub confidence_reason: String,
}

// ===== Signal Event types (V2 Signal Engine) =====

/// Fixed set of signal event types — prevents string inconsistency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalEventType {
    Created,
    ScoreChanged,
    EvidenceAdded,
    EntityAdded,
    StatusChanged,
    Accelerating,
    Decaying,
    Resolved,
}

impl std::fmt::Display for SignalEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::ScoreChanged => write!(f, "score_changed"),
            Self::EvidenceAdded => write!(f, "evidence_added"),
            Self::EntityAdded => write!(f, "entity_added"),
            Self::StatusChanged => write!(f, "status_changed"),
            Self::Accelerating => write!(f, "accelerating"),
            Self::Decaying => write!(f, "decaying"),
            Self::Resolved => write!(f, "resolved"),
        }
    }
}

/// A single timeline event for a signal thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalEvent {
    pub id: i64,
    pub thread_id: i64,
    pub event_type: String,
    pub payload: Option<String>,
    pub created_at: i64,
}
