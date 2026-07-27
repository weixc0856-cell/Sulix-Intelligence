//! Reasoning Framework — the core domain entity
//!
//! A ReasoningFramework is a structured mental model that guides how Sulix
//! interprets evidence and forms judgments. Each framework has:
//!
//! - **Trigger rules**: when this framework should be considered
//! - **Reasoning template**: how to apply this framework in LLM prompts
//! - **Calibration data**: how accurate this framework has been historically

use serde::{Deserialize, Serialize};

/// Category of reasoning framework.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrameworkCategory {
    MathematicalModels,
    FinancialIntelligence,
    HumanBehavior,
    StrategicModels,
    SystemsThinking,
    ScientificThinking,
}

impl FrameworkCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::MathematicalModels => "Mathematics",
            Self::FinancialIntelligence => "Finance",
            Self::HumanBehavior => "Human Behavior",
            Self::StrategicModels => "Strategy",
            Self::SystemsThinking => "Systems Thinking",
            Self::ScientificThinking => "Scientific Thinking",
        }
    }
}

/// A trigger rule defines when a framework should be selected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerRule {
    /// Optional signal type match (e.g. "entity_signal", "claim")
    pub signal_type: Option<String>,
    /// Optional entity type match (e.g. "company", "technology", "policy")
    pub entity_type: Option<String>,
    /// Optional question type match (e.g. "adoption", "valuation", "risk")
    pub question_type: Option<String>,
    /// Fallback keyword matching
    #[serde(default)]
    pub keywords: Vec<String>,
}

impl TriggerRule {
    pub fn matches(&self, signal_type: Option<&str>, entity_type: Option<&str>, question_type: Option<&str>, keywords: &[&str]) -> bool {
        // Direct type matches (high precision)
        if let Some(st) = &self.signal_type {
            if signal_type.map_or(false, |s| s == st) { return true; }
        }
        if let Some(et) = &self.entity_type {
            if entity_type.map_or(false, |e| e == et) { return true; }
        }
        if let Some(qt) = &self.question_type {
            if question_type.map_or(false, |q| q == qt) { return true; }
        }
        // Keyword fallback
        if !self.keywords.is_empty() {
            if keywords.iter().any(|k| self.keywords.iter().any(|kw| kw.eq_ignore_ascii_case(k))) {
                return true;
            }
        }
        false
    }
}

/// A reasoning framework — a structured mental model for judgment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningFramework {
    /// Unique identifier, e.g. "compound-growth"
    pub id: String,
    /// Human-readable name, e.g. "Compound Growth Analysis"
    pub name: String,
    /// Category
    pub category: FrameworkCategory,
    /// 2-3 sentence explanation
    pub description: String,
    /// Trigger rules for automatic selection
    #[serde(default)]
    pub trigger_rules: Vec<TriggerRule>,
    /// Reasoning template for LLM prompt injection
    pub reasoning_template: String,
    /// Evidence types this framework requires
    #[serde(default)]
    pub evidence_requirements: Vec<String>,
    // ── Calibration data (updated by outcome feedback) ──
    /// Historical accuracy score (0.0 – 1.0)
    pub calibration_score: f64,
    /// Number of times this framework has been used
    pub usage_count: u64,
    /// Average confidence delta when this framework is applied
    pub confidence_delta_avg: f64,
}

/// Input for creating a new framework.
#[derive(Debug, Clone)]
pub struct NewFramework {
    pub id: String,
    pub name: String,
    pub category: FrameworkCategory,
    pub description: String,
    pub trigger_rules: Vec<TriggerRule>,
    pub reasoning_template: String,
    pub evidence_requirements: Vec<String>,
}

impl From<NewFramework> for ReasoningFramework {
    fn from(n: NewFramework) -> Self {
        Self {
            id: n.id,
            name: n.name,
            category: n.category,
            description: n.description,
            trigger_rules: n.trigger_rules,
            reasoning_template: n.reasoning_template,
            evidence_requirements: n.evidence_requirements,
            calibration_score: 0.0,
            usage_count: 0,
            confidence_delta_avg: 0.0,
        }
    }
}

/// A framework applied to a specific claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimFrameworkRef {
    pub claim_id: i64,
    pub framework_id: String,
    pub relevance: f64,
    pub reasoning: String,
    pub confidence_before: Option<f64>,
    pub confidence_after: Option<f64>,
}

/// Impact of applying a framework to a claim.
#[derive(Debug, Clone)]
pub struct FrameworkImpact {
    pub framework_id: String,
    pub claim_id: i64,
    pub confidence_before: f64,
    pub confidence_after: f64,
    pub delta: f64,
}

impl FrameworkImpact {
    pub fn new(framework_id: String, claim_id: i64, confidence_before: f64, confidence_after: f64) -> Self {
        Self { framework_id, claim_id, confidence_before, confidence_after, delta: confidence_after - confidence_before }
    }
}
