//! Reasoning Selector — rule-based framework matching
//!
//! Selects applicable reasoning frameworks based on signal type, entity type,
//! question type, and keyword fallback. This is the **first pass** — the LLM
//! refines selection during claim extraction.

use crate::error::FrameworkError;
use crate::framework::ReasoningFramework;
use crate::repository::FrameworkRepository;

/// Selects applicable reasoning frameworks for a given problem context.
pub struct ReasoningSelector<R: FrameworkRepository> {
    repo: R,
}

impl<R: FrameworkRepository> ReasoningSelector<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// Select frameworks matching the given context.
    ///
    /// - `signal_type`: "entity_signal", "claim", "observation"
    /// - `entity_type`: "company", "technology", "policy", "market", "platform"
    /// - `question_type`: "adoption", "valuation", "risk", "competition", "growth"
    /// - `keywords`: additional keywords from the article/signal context
    pub async fn select(
        &self,
        signal_type: Option<&str>,
        entity_type: Option<&str>,
        question_type: Option<&str>,
        keywords: &[&str],
    ) -> Result<Vec<ReasoningFramework>, FrameworkError> {
        let all = self.repo.list_all().await?;

        let matched: Vec<ReasoningFramework> = all
            .into_iter()
            .filter(|fw| {
                fw.trigger_rules.iter().any(|rule| rule.matches(signal_type, entity_type, question_type, keywords))
            })
            .collect();

        Ok(matched)
    }

    /// Select frameworks by category (direct user request).
    pub async fn by_category(&self, category: &str) -> Result<Vec<ReasoningFramework>, FrameworkError> {
        // Parse category from string
        let cat = match category.to_lowercase().as_str() {
            "mathematics" | "mathematical" => crate::framework::FrameworkCategory::MathematicalModels,
            "finance" | "financial" => crate::framework::FrameworkCategory::FinancialIntelligence,
            "behavior" | "human_behavior" | "psychology" => crate::framework::FrameworkCategory::HumanBehavior,
            "strategy" | "strategic" => crate::framework::FrameworkCategory::StrategicModels,
            "systems" | "systems_thinking" => crate::framework::FrameworkCategory::SystemsThinking,
            "science" | "scientific" => crate::framework::FrameworkCategory::ScientificThinking,
            _ => return Ok(Vec::new()),
        };
        self.repo.list_by_category(cat).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::{FrameworkCategory, NewFramework, TriggerRule};
    use crate::repository::FrameworkRepository;
    use async_trait::async_trait;

    struct MemRepo {
        frameworks: Vec<ReasoningFramework>,
    }

    #[async_trait(?Send)]
    impl FrameworkRepository for MemRepo {
        async fn find(&self, _id: &str) -> Result<Option<ReasoningFramework>, FrameworkError> {
            Ok(None)
        }
        async fn list_by_category(&self, _cat: FrameworkCategory) -> Result<Vec<ReasoningFramework>, FrameworkError> {
            Ok(self.frameworks.clone())
        }
        async fn list_all(&self) -> Result<Vec<ReasoningFramework>, FrameworkError> {
            Ok(self.frameworks.clone())
        }
        async fn search(&self, _query: &str) -> Result<Vec<ReasoningFramework>, FrameworkError> {
            Ok(Vec::new())
        }
        async fn seed(&self, _fw: &[ReasoningFramework]) -> Result<(), FrameworkError> {
            Ok(())
        }
        async fn update_calibration(
            &self,
            _id: &str,
            _score: f64,
            _count: u64,
            _delta: f64,
        ) -> Result<(), FrameworkError> {
            Ok(())
        }
    }

    fn make_repo() -> MemRepo {
        let fw = ReasoningFramework::from(NewFramework {
            id: "compound-growth".into(),
            name: "Compound Growth".into(),
            category: FrameworkCategory::FinancialIntelligence,
            description: "Small continuous growth leads to exponential results".into(),
            trigger_rules: vec![TriggerRule {
                signal_type: Some("entity_signal".into()),
                entity_type: Some("company".into()),
                question_type: Some("growth".into()),
                keywords: vec!["compound".into(), "exponential".into(), "growth".into()],
            }],
            reasoning_template: "Consider whether growth is linear or compound...".into(),
            evidence_requirements: vec!["growth_rate".into(), "time_horizon".into()],
        });
        MemRepo { frameworks: vec![fw] }
    }

    #[test]
    fn selector_matches_by_entity_type() {
        let repo = make_repo();
        let selector = ReasoningSelector::new(repo);
        let result =
            futures::executor::block_on(selector.select(Some("entity_signal"), Some("company"), Some("growth"), &[]));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn selector_no_match() {
        let repo = make_repo();
        let selector = ReasoningSelector::new(repo);
        let result =
            futures::executor::block_on(selector.select(Some("observation"), Some("weather"), Some("climate"), &[]));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }
}
