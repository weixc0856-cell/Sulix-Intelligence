use crate::types::{CognitiveStage, DesiredOutcome, Intent};

/// Rule-based IntentParser (MVP). Maps query text to structured Intent.
/// Future: LLM-based parser with same interface.
pub fn parse(query: &str) -> Intent {
    let q = query.to_lowercase();
    let (stage, desired_outcome) =
        if q.contains("should i") || q.contains("invest") || q.contains("buy") || q.contains("enter") {
            (CognitiveStage::Explore, DesiredOutcome::Recommendation)
        } else if q.contains("why did")
            || q.contains("why does")
            || q.contains("why is")
            || q.contains("fail")
            || q.contains("what did i learn")
            || q.contains("lesson")
            || q.contains("reflect")
        {
            (CognitiveStage::Review, DesiredOutcome::Explanation)
        } else {
            (CognitiveStage::Learn, DesiredOutcome::Explanation)
        };

    let intent_type = match stage {
        CognitiveStage::Explore | CognitiveStage::Decide => "decision_support",
        CognitiveStage::Review => "reflection",
        CognitiveStage::Learn => "pattern_analysis",
    };

    let domain = if q.contains("invest") || q.contains("startup") || q.contains("market") || q.contains("ai") {
        Some("investment".to_string())
    } else if q.contains("career") || q.contains("job") || q.contains("work") {
        Some("career".to_string())
    } else if q.contains("product") || q.contains("build") || q.contains("launch") {
        Some("product".to_string())
    } else {
        None
    };

    let action = match desired_outcome {
        DesiredOutcome::Recommendation => Some("evaluate".to_string()),
        DesiredOutcome::Explanation => Some("understand".to_string()),
        DesiredOutcome::Comparison => Some("compare".to_string()),
        DesiredOutcome::Prediction => Some("predict".to_string()),
    };

    Intent { intent_type: intent_type.into(), stage, desired_outcome, domain, action, entity: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn investment_query_detects_domain() {
        let i = parse("Should I invest in AI startups?");
        assert_eq!(i.intent_type, "decision_support");
        assert_eq!(i.stage, CognitiveStage::Explore);
        assert_eq!(i.domain.as_deref(), Some("investment"));
    }

    #[test]
    fn failure_query_detects_review() {
        let i = parse("Why did my last startup fail?");
        assert_eq!(i.intent_type, "reflection");
        assert_eq!(i.stage, CognitiveStage::Review);
    }

    #[test]
    fn learning_query_falls_back() {
        let i = parse("Tell me about the market trends");
        assert_eq!(i.intent_type, "pattern_analysis");
    }

    #[test]
    fn career_query_detects_domain() {
        let i = parse("Should I change my career?");
        assert_eq!(i.domain.as_deref(), Some("career"));
    }
}
