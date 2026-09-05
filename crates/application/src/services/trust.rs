//! Trust application service — assembles the trust-dashboard report that the
//! `/api/.../trust` route exposes.
//!
//! Generic over the four store surfaces it reads:
//! [`store::FeedQueryService`] (health), [`store::DecisionQueryService`]
//! (decision stats), [`store::SourceQueryService`] (source registry) and
//! [`store::MetricsStore`] (model / calibration / decision-outcome stats).
//!
//! Every store read is best-effort: a failing metric simply reports a zero /
//! empty baseline rather than failing the whole dashboard (mirrors the
//! historical handler behaviour).  The report is returned as JSON because the
//! metric legs are already loose row JSON from D1.

use serde_json::json;

use store::{EvalSummary, Source};

/// Application service for the trust-dashboard use-case.
pub struct TrustService<S> {
    store: S,
}

impl<S> TrustService<S>
where
    S: store::FeedQueryService + store::DecisionQueryService + store::SourceQueryService + store::MetricsStore,
{
    /// Wrap a store in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Build the trust report.
    pub async fn build(&self) -> Result<serde_json::Value, store::StoreError> {
        let health = self.store.health_stats().await.ok();
        let decision_stats = self.store.decision_stats().await.ok();

        let sources = self.store.list_sources(None, None, 100, 0).await.unwrap_or_default();
        let source_reliability = source_reliability(&sources);
        let total_sources = sources.len();

        let accuracy_rate = decision_stats.as_ref().and_then(|ds| accuracy_rate(&ds.evaluation_summary));

        // Model-invocation stats from reasoning_runs, calibration rows and
        // decision-record/outcome aggregates — best-effort, zero baseline.
        let model_stats = self.store.model_reliability_stats().await.unwrap_or_default();
        let calibration_stats = self.store.calibration_stats().await.unwrap_or_default();
        let decision_accuracy = self.store.decision_accuracy_stats().await.unwrap_or_default();
        let outcome_success = self.store.outcome_success_stats().await.unwrap_or_default();

        Ok(json!({
            "signals_analyzed": health.as_ref().map(|h| h.article_count).unwrap_or(0),
            "active_sources": total_sources,
            "total_decisions": decision_stats.as_ref().map(|ds| ds.total_decisions).unwrap_or(0),
            "total_evaluations": decision_stats.as_ref().map(|ds| ds.evaluation_summary.total_evaluated).unwrap_or(0),
            "accuracy_rate": accuracy_rate,
            "source_reliability": source_reliability,
            "evaluation_summary": decision_stats.map(|ds| ds.evaluation_summary),
            "model_reliability": model_stats,
            "calibration": calibration_stats,
            "decision_accuracy": decision_accuracy,
            "outcome_success": outcome_success,
        }))
    }
}

/// Project sources carrying a trust score onto the reliability report rows.
fn source_reliability(sources: &[Source]) -> Vec<serde_json::Value> {
    sources
        .iter()
        .filter(|s| s.trust_score.is_some())
        .map(|s| {
            json!({
                "name": s.name.as_deref().unwrap_or("Unknown"),
                "tier": s.tier,
                "trust_score": s.trust_score.unwrap_or(0.0),
                "verified": s.verified,
                "policy": s.policy,
            })
        })
        .collect()
}

/// Share of evaluated decisions that were confirmed; `None` when nothing has
/// been evaluated yet.
fn accuracy_rate(summary: &EvalSummary) -> Option<f64> {
    let total = summary.total_evaluated;
    if total > 0 {
        Some(summary.confirmed as f64 / total as f64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    fn new_source(name: &str, tier: &str, policy: &str, trust_score: Option<f64>, verified: bool) -> Source {
        Source {
            id: 0,
            source_type: "rss".into(),
            feed_id: None,
            name: Some(name.into()),
            tier: tier.into(),
            policy: policy.into(),
            license: "public_domain".into(),
            license_detail: None,
            attribution: None,
            trust_score,
            retention_days: None,
            verified,
            notes: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn source_reliability_drops_unscored_sources() {
        let sources = vec![
            new_source("acme", "Tier0", "FullTextAllowed", Some(0.9), true),
            new_source("scoredless", "Tier2", "SummaryAllowed", None, false),
            new_source("also", "Tier1", "SummaryAllowed", Some(0.4), false),
        ];
        let rows = source_reliability(&sources);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["name"], "acme");
        assert_eq!(rows[0]["trust_score"], 0.9);
        assert_eq!(rows[1]["tier"], "Tier1");
    }

    #[test]
    fn accuracy_rate_is_share_of_confirmed() {
        let summary = EvalSummary {
            total_evaluated: 10,
            confirmed: 7,
            partially_confirmed: 2,
            contradicted: 1,
            inconclusive: 0,
            accuracy_rate: 0.7,
        };
        let rate = accuracy_rate(&summary).expect("non-empty evaluations yield a rate");
        assert!((rate - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn accuracy_rate_none_when_nothing_evaluated() {
        let summary = EvalSummary {
            total_evaluated: 0,
            confirmed: 0,
            partially_confirmed: 0,
            contradicted: 0,
            inconclusive: 0,
            accuracy_rate: 0.0,
        };
        assert!(accuracy_rate(&summary).is_none());
    }

    #[test]
    fn build_assembles_report_over_empty_store() {
        let svc = TrustService::new(MemoryStore::new());
        let report = futures::executor::block_on(svc.build()).expect("build is best-effort and never fails");
        assert_eq!(report["signals_analyzed"], 0);
        assert_eq!(report["active_sources"], 0);
        assert!(report["source_reliability"].as_array().is_some());
        assert!(report["model_reliability"].as_array().is_some());
        assert!(report["calibration"].as_array().is_some());
        // The single-object metric legs are Null over MemoryStore (unmodeled),
        // so only assert the keys are present in the assembled report.
        assert!(report.get("decision_accuracy").is_some());
        assert!(report.get("outcome_success").is_some());
    }
}
