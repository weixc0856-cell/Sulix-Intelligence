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
    pub fn matches(
        &self,
        signal_type: Option<&str>,
        entity_type: Option<&str>,
        question_type: Option<&str>,
        keywords: &[&str],
    ) -> bool {
        // AND-within-rule: ALL non-None fields must match
        if let Some(st) = &self.signal_type {
            if !signal_type.is_some_and(|s| s == st) {
                return false;
            }
        }
        if let Some(et) = &self.entity_type {
            if !entity_type.is_some_and(|e| e == et) {
                return false;
            }
        }
        if let Some(qt) = &self.question_type {
            if !question_type.is_some_and(|q| q == qt) {
                return false;
            }
        }
        // Keywords: if both rule and input have keywords, at least one must match
        if !self.keywords.is_empty()
            && !keywords.is_empty()
            && !keywords.iter().any(|k| self.keywords.iter().any(|kw| kw.eq_ignore_ascii_case(k)))
        {
            return false;
        }
        // If at least one field is set and all set fields match → true
        self.signal_type.is_some()
            || self.entity_type.is_some()
            || self.question_type.is_some()
            || !self.keywords.is_empty()
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
        Self {
            framework_id,
            claim_id,
            confidence_before,
            confidence_after,
            delta: confidence_after - confidence_before,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── FrameworkCategory ──

    #[test]
    fn category_labels_are_human_readable() {
        assert_eq!(FrameworkCategory::MathematicalModels.label(), "Mathematics");
        assert_eq!(FrameworkCategory::FinancialIntelligence.label(), "Finance");
        assert_eq!(FrameworkCategory::HumanBehavior.label(), "Human Behavior");
        assert_eq!(FrameworkCategory::StrategicModels.label(), "Strategy");
        assert_eq!(FrameworkCategory::SystemsThinking.label(), "Systems Thinking");
        assert_eq!(FrameworkCategory::ScientificThinking.label(), "Scientific Thinking");
    }

    // ── TriggerRule matching (AND-within-rule, OR-across-rules) ──

    /// A rule with all fields set (signal + entity + question + keywords)
    fn make_full_rule() -> TriggerRule {
        TriggerRule {
            signal_type: Some("entity_signal".into()),
            entity_type: Some("company".into()),
            question_type: Some("growth".into()),
            keywords: vec!["compound".into(), "exponential".into()],
        }
    }

    fn make_single_entity_rule() -> TriggerRule {
        TriggerRule { signal_type: None, entity_type: Some("company".into()), question_type: None, keywords: vec![] }
    }

    #[test]
    fn single_field_rule_matches_when_field_matches() {
        let rule = make_single_entity_rule();
        assert!(rule.matches(None, Some("company"), None, &[]));
    }

    #[test]
    fn single_field_rule_rejects_when_field_doesnt_match() {
        let rule = make_single_entity_rule();
        assert!(!rule.matches(None, Some("technology"), None, &[]));
    }

    #[test]
    fn multi_field_requires_all_to_match() {
        let rule = make_full_rule();
        // All type fields match, keywords also match
        assert!(rule.matches(Some("entity_signal"), Some("company"), Some("growth"), &["compound"]));
    }

    #[test]
    fn multi_field_skips_keyword_check_when_no_input_keywords() {
        let rule = make_full_rule();
        // Type fields all match, empty keywords input → skip keyword check
        assert!(rule.matches(Some("entity_signal"), Some("company"), Some("growth"), &[]));
    }

    #[test]
    fn multi_field_skips_keyword_check_when_no_rule_keywords() {
        let rule = TriggerRule {
            signal_type: Some("entity_signal".into()),
            entity_type: Some("company".into()),
            question_type: None,
            keywords: vec![],
        };
        // Type fields match, empty rule keywords → skip keyword check
        assert!(rule.matches(Some("entity_signal"), Some("company"), None, &["anything"]));
    }

    #[test]
    fn multi_field_rejects_when_one_field_mismatches() {
        let rule = make_full_rule();
        // signal_type matches, entity_type matches, but question_type doesn't
        assert!(!rule.matches(Some("entity_signal"), Some("company"), Some("valuation"), &["compound"]));
    }

    #[test]
    fn multi_field_rejects_when_keyword_mismatches() {
        let rule = make_full_rule();
        assert!(!rule.matches(Some("entity_signal"), Some("company"), Some("growth"), &["linear"]));
    }

    #[test]
    fn rule_with_only_keywords_matches_by_keyword() {
        let rule = TriggerRule {
            signal_type: None,
            entity_type: None,
            question_type: None,
            keywords: vec!["ai".into(), "ml".into()],
        };
        assert!(rule.matches(None, None, None, &["AI"]));
        assert!(!rule.matches(None, None, None, &["blockchain"]));
    }

    #[test]
    fn rule_with_no_fields_set_matches_nothing() {
        let rule = TriggerRule { signal_type: None, entity_type: None, question_type: None, keywords: vec![] };
        assert!(!rule.matches(Some("anything"), None, None, &[]));
    }

    // ── FrameworkImpact ──

    #[test]
    fn impact_delta_is_confidence_after_minus_before() {
        let impact = FrameworkImpact::new("test-fw".into(), 1, 0.7, 0.85);
        assert!((impact.delta - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn impact_delta_can_be_negative() {
        let impact = FrameworkImpact::new("test-fw".into(), 1, 0.8, 0.65);
        assert!((impact.delta - (-0.15)).abs() < f64::EPSILON);
    }

    // ── NewFramework → ReasoningFramework conversion ──

    #[test]
    fn new_framework_converts_with_default_calibration() {
        let new = NewFramework {
            id: "test-fw".into(),
            name: "Test Framework".into(),
            category: FrameworkCategory::StrategicModels,
            description: "A test framework".into(),
            trigger_rules: vec![],
            reasoning_template: "Test template".into(),
            evidence_requirements: vec![],
        };
        let fw: ReasoningFramework = new.into();
        assert_eq!(fw.id, "test-fw");
        assert_eq!(fw.calibration_score, 0.0);
        assert_eq!(fw.usage_count, 0);
        assert_eq!(fw.confidence_delta_avg, 0.0);
    }
}
