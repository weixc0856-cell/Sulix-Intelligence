//! Calibration Engine — updates framework accuracy based on outcome feedback
//!
//! Each time a decision using a framework receives an outcome, the calibration
//! score is updated. Over time, this reveals which frameworks are genuinely
//! predictive vs. which are merely plausible.

use crate::error::FrameworkError;
use crate::framework::FrameworkImpact;
use crate::repository::FrameworkRepository;

/// CalibrationEngine updates framework scores after outcomes are recorded.
pub struct CalibrationEngine<R: FrameworkRepository> {
    repo: R,
}

impl<R: FrameworkRepository> CalibrationEngine<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// Record an outcome for a set of framework impacts.
    ///
    /// `outcome_achieved`: `true` if the decision's prediction was correct.
    /// `impacts`: the framework impacts recorded when the decision was made.
    ///
    /// Updates each framework's calibration_score, usage_count, and
    /// confidence_delta_avg.
    pub async fn record_outcome(
        &self,
        outcome_achieved: bool,
        impacts: &[FrameworkImpact],
    ) -> Result<(), FrameworkError> {
        for impact in impacts {
            let fw = self.repo.find(&impact.framework_id).await?;
            if let Some(framework) = fw {
                let new_usage = framework.usage_count + 1;
                let outcome_score = if outcome_achieved { 1.0 } else { 0.0 };
                let new_calibration =
                    (framework.calibration_score * framework.usage_count as f64 + outcome_score) / new_usage as f64;
                let new_delta_avg =
                    (framework.confidence_delta_avg * framework.usage_count as f64 + impact.delta) / new_usage as f64;

                self.repo.update_calibration(&impact.framework_id, new_calibration, new_usage, new_delta_avg).await?;
            }
        }
        Ok(())
    }

    /// Get frameworks sorted by calibration score (most accurate first).
    pub async fn top_frameworks(&self, min_usage: u64) -> Result<Vec<String>, FrameworkError> {
        let all = self.repo.list_all().await?;
        let mut sorted: Vec<_> = all.into_iter().filter(|fw| fw.usage_count >= min_usage).collect();
        sorted
            .sort_by(|a, b| b.calibration_score.partial_cmp(&a.calibration_score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(sorted.into_iter().map(|fw| fw.id).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::{FrameworkCategory, NewFramework, ReasoningFramework};
    use crate::repository::FrameworkRepository;
    use async_trait::async_trait;
    use std::cell::RefCell;

    struct TrackingRepo {
        frameworks: RefCell<Vec<ReasoningFramework>>,
        updates: RefCell<Vec<(String, f64, u64, f64)>>,
    }

    #[async_trait(?Send)]
    impl FrameworkRepository for TrackingRepo {
        async fn find(&self, id: &str) -> Result<Option<ReasoningFramework>, FrameworkError> {
            Ok(self.frameworks.borrow().iter().find(|fw| fw.id == id).cloned())
        }
        async fn list_by_category(&self, _cat: FrameworkCategory) -> Result<Vec<ReasoningFramework>, FrameworkError> {
            Ok(self.frameworks.borrow().clone())
        }
        async fn list_all(&self) -> Result<Vec<ReasoningFramework>, FrameworkError> {
            Ok(self.frameworks.borrow().clone())
        }
        async fn search(&self, _query: &str) -> Result<Vec<ReasoningFramework>, FrameworkError> {
            Ok(Vec::new())
        }
        async fn seed(&self, _fw: &[ReasoningFramework]) -> Result<(), FrameworkError> {
            Ok(())
        }
        async fn update_calibration(&self, id: &str, score: f64, count: u64, delta: f64) -> Result<(), FrameworkError> {
            self.updates.borrow_mut().push((id.to_string(), score, count, delta));
            Ok(())
        }
    }

    fn make_repo() -> TrackingRepo {
        let fw = ReasoningFramework::from(NewFramework {
            id: "test-fw".into(),
            name: "Test Framework".into(),
            category: FrameworkCategory::FinancialIntelligence,
            description: "Test".into(),
            trigger_rules: vec![],
            reasoning_template: "".into(),
            evidence_requirements: vec![],
        });
        TrackingRepo { frameworks: RefCell::new(vec![fw]), updates: RefCell::new(Vec::new()) }
    }

    #[test]
    fn calibration_updates_on_outcome() {
        let repo = make_repo();
        let engine = CalibrationEngine::new(repo);
        let impacts = vec![FrameworkImpact::new("test-fw".into(), 1, 0.7, 0.82)];
        let result = futures::executor::block_on(engine.record_outcome(true, &impacts));
        assert!(result.is_ok());
    }
}
