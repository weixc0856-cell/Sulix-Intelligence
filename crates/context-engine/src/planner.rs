use crate::types::{CognitiveStage, DecisionQuery, Intent, MemoryQuery, ReflectionQuery, RetrievalPlan};

/// RetrievalPlanner — maps Intent to a structured retrieval plan.
/// Decides WHAT to retrieve; Retriever executes.
pub fn plan(intent: &Intent) -> RetrievalPlan {
    let domain = intent.domain.as_deref();
    let stage = &intent.stage;

    let decision_query =
        Some(DecisionQuery { domain: domain.map(String::from), status: Some("active".into()), limit: 10 });

    let reflection_query = match stage {
        CognitiveStage::Review => {
            Some(ReflectionQuery { status: Some("generated".into()), min_quality: Some(0.5), limit: 10 })
        }
        _ => Some(ReflectionQuery { status: Some("generated".into()), min_quality: None, limit: 5 }),
    };

    let memory_query = Some(MemoryQuery {
        memory_types: vec!["strategic_pattern".into(), "decision_heuristic".into(), "domain_knowledge".into()],
        status: Some("active".into()),
        min_confidence: None,
        limit: 10,
    });

    let pattern_enabled = matches!(stage, CognitiveStage::Review | CognitiveStage::Learn);

    RetrievalPlan { decision_query, reflection_query, memory_query, pattern_enabled, max_results: 10 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CognitiveStage, DesiredOutcome};

    #[test]
    fn investment_intent_plans_decision_query() {
        let intent = Intent {
            intent_type: "decision_support".into(),
            stage: CognitiveStage::Explore,
            desired_outcome: DesiredOutcome::Recommendation,
            domain: Some("investment".into()),
            action: Some("evaluate".into()),
            entity: None,
        };
        let plan = plan(&intent);
        assert_eq!(plan.decision_query.as_ref().unwrap().domain.as_deref(), Some("investment"));
        assert!(!plan.pattern_enabled);
    }

    #[test]
    fn review_intent_enables_patterns() {
        let intent = Intent {
            intent_type: "reflection".into(),
            stage: CognitiveStage::Review,
            desired_outcome: DesiredOutcome::Explanation,
            domain: None,
            action: Some("understand".into()),
            entity: None,
        };
        let plan = plan(&intent);
        assert!(plan.pattern_enabled);
    }
}
