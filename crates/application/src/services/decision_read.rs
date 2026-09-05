//! Decision read-model application service.
//!
//! Backs the `/api/intelligence/decisions*` read endpoints (list, detail,
//! by-signal, stats, evaluations, outcomes, timeline, explanation) and the
//! `/api/decision-records*` read/memo endpoints (records list, record detail,
//! record outcomes, framework traces, memo persistence).
//!
//! The GATED decision-write vertical (create / status / outcome / evaluation)
//! deliberately stays out of this service — it is relocated whole in a later
//! checkpoint.  Here every method is a pure D1 read (or a read composition);
//! `decision_engine::generate_memo` (pure CPU) also stays with the caller.
//!
//! Zero Worker / HTTP / `js_sys` code; unit-testable with `MemoryStore`.

use domain::{Decision, DecisionEvaluation, DecisionOutcome, DecisionRecord, DecisionStats, OutcomeEvent, StoreError};

/// One event on a decision's timeline.
#[derive(Debug, serde::Serialize)]
pub struct DecisionTimelineEvent {
    pub timestamp: i64,
    pub event_type: String,
    pub title: String,
    pub description: String,
}

/// Assembled timeline for a decision (events + latest reflection learning).
#[derive(Debug, serde::Serialize)]
pub struct DecisionTimeline {
    pub events: Vec<DecisionTimelineEvent>,
    pub learning: Option<String>,
}

/// A supporting article surfaced in a decision explanation.
#[derive(Debug, serde::Serialize)]
pub struct SupportingEvidence {
    pub title: String,
    pub strength: f64,
    pub source: Option<String>,
}

/// A named factor that raised/lowered confidence.
#[derive(Debug, serde::Serialize)]
pub struct ConfidenceDriver {
    pub factor: String,
    pub impact: String,
}

/// A reasoning-framework trace applied to the decision's claims.
#[derive(Debug, serde::Serialize)]
pub struct FrameworkTrace {
    pub id: String,
    pub name: String,
    pub category: String,
    pub relevance: f64,
    pub reasoning: String,
}

/// Structured explanation of why the system holds this belief.
#[derive(Debug, serde::Serialize)]
pub struct DecisionExplanation {
    pub decision_id: String,
    pub decision_title: String,
    pub hypothesis: Option<String>,
    pub confidence: f64,
    pub trend: String,
    pub supporting_evidence: Vec<SupportingEvidence>,
    pub confidence_drivers: Vec<ConfidenceDriver>,
    pub uncertainties: Vec<String>,
    pub outcome_summary: Option<String>,
    pub frameworks_applied: Vec<FrameworkTrace>,
}

/// Verifiable decision record + its outcome metrics and linked claims.
#[derive(Debug, serde::Serialize)]
pub struct DecisionRecordDetail {
    pub record: DecisionRecord,
    pub outcomes: Vec<DecisionOutcome>,
    pub claims: Vec<serde_json::Value>,
}

/// Application service for the decision / decision-record read use-cases.
pub struct DecisionReadService<S> {
    store: S,
}

impl<S> DecisionReadService<S>
where
    S: domain::DecisionQueryService
        + domain::DecisionRepository
        + domain::ReflectionPersistence
        + domain::SignalStore
        + domain::ConfidenceRepository
        + domain::DecisionRecordStore,
{
    /// Wrap a store (or store-backed query-service set) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    // ── decisions ──

    /// List decisions (optionally by status) — `/api/intelligence/decisions`.
    pub async fn list(&self, status: Option<&str>, limit: u32) -> Result<Vec<Decision>, StoreError> {
        self.store.list_decisions(status, limit).await
    }

    /// Decision detail; `Ok(None)` when no such decision — GET `:id`.
    pub async fn detail(&self, id: i64) -> Result<Option<Decision>, StoreError> {
        self.store.find_decision(id).await
    }

    /// Decisions attached to a signal thread — GET `/signals/:id/decisions`.
    pub async fn by_signal(&self, signal_thread_id: i64) -> Result<Vec<Decision>, StoreError> {
        self.store.decisions_by_signal(signal_thread_id).await
    }

    /// Decision accuracy dashboard aggregates — GET `decisions/stats`.
    pub async fn stats(&self) -> Result<DecisionStats, StoreError> {
        self.store.decision_stats().await
    }

    /// Outcome events for a decision — GET `decisions/:id/outcomes`.
    pub async fn list_outcomes(&self, decision_id: i64) -> Result<Vec<OutcomeEvent>, StoreError> {
        self.store.list_outcomes(decision_id).await
    }

    /// Evaluations for a decision — GET `decisions/:id/evaluations`.
    pub async fn list_evaluations(&self, decision_id: i64) -> Result<Vec<DecisionEvaluation>, StoreError> {
        self.store.list_evaluations(decision_id).await
    }

    /// Assembled chronological timeline for a decision — GET `decisions/:id/timeline`.
    /// `Ok(None)` when no such decision.  Sub-read failures are soft (mirroring
    /// the previous route behaviour): missing outcomes/evaluations/reflection
    /// simply contribute no events.
    pub async fn timeline(&self, id: i64) -> Result<Option<DecisionTimeline>, StoreError> {
        let decision = match self.store.find_decision(id).await {
            Ok(Some(d)) => d,
            Ok(None) => return Ok(None),
            Err(e) => return Err(e),
        };

        let mut events = vec![DecisionTimelineEvent {
            timestamp: decision.created_at,
            event_type: "decision.created".into(),
            title: "Decision registered".into(),
            description: format!("Status: {}, Confidence: {:.0}%", decision.status, decision.confidence * 100.0),
        }];

        if let Ok(outcomes) = self.store.list_outcomes(id).await {
            for o in &outcomes {
                events.push(DecisionTimelineEvent {
                    timestamp: o.observed_at,
                    event_type: "outcome.observed".into(),
                    title: format!("Outcome: {}", o.outcome_type),
                    description: o.observation.clone(),
                });
            }
        }

        if let Ok(evals) = self.store.list_evaluations(id).await {
            for e in &evals {
                events.push(DecisionTimelineEvent {
                    timestamp: e.evaluated_at,
                    event_type: "decision.evaluated".into(),
                    title: format!("Judgment: {}", e.evaluation),
                    description: e.reasoning.clone().unwrap_or_default(),
                });
            }
        }

        if let Ok(Some(r)) = self.store.get_reflection_by_decision(id).await {
            if let Some(started) = r.started_at {
                events.push(DecisionTimelineEvent {
                    timestamp: started,
                    event_type: "reflection.generated".into(),
                    title: "AI Reflection".into(),
                    description: r.result.unwrap_or_default(),
                });
            }
        }

        events.sort_by_key(|e| e.timestamp);
        let learning = self.store.get_reflection_by_decision(id).await.ok().flatten().and_then(|r| r.result);

        Ok(Some(DecisionTimeline { events, learning }))
    }

    /// Structured "why Sulix thinks this" explanation — GET `decisions/:id/explanation`.
    /// `Ok(None)` when no such decision.  Evidence / outcome / framework
    /// sub-reads are soft and degrade to empty sections, as the previous route
    /// behaviour did.
    pub async fn explanation(&self, id: i64) -> Result<Option<DecisionExplanation>, StoreError> {
        let decision = match self.store.find_decision(id).await {
            Ok(Some(d)) => d,
            Ok(None) => return Ok(None),
            Err(e) => return Err(e),
        };

        // Determine trend from confidence history
        let trend = match self.store.list_confidence_history("decision", &id.to_string()).await {
            Ok(events) if events.len() >= 2 => {
                let latest = events.last().unwrap();
                let prev = &events[events.len() - 2];
                if latest.confidence > prev.previous_confidence.unwrap_or(0.0) {
                    "increasing".into()
                } else if latest.confidence < prev.confidence {
                    "decreasing".into()
                } else {
                    "stable".into()
                }
            }
            _ => "stable".into(),
        };

        // Load evidence from signal thread
        let mut supporting_evidence: Vec<SupportingEvidence> = Vec::new();
        if let Some(signal_id) = decision.signal_thread_id {
            if let Ok(Some(detail)) = self.store.load_signal_detail(signal_id).await {
                for article in &detail.evidence_top {
                    supporting_evidence.push(SupportingEvidence {
                        title: article.title.clone(),
                        strength: article.score.clamp(0.0, 1.0),
                        source: article.feed_name.clone(),
                    });
                }
            }
        }

        // Load outcomes for accuracy summary
        let outcome_summary = match self.store.list_outcomes(id).await {
            Ok(outcomes) if !outcomes.is_empty() => {
                let confirmed = outcomes
                    .iter()
                    .filter(|o| o.outcome_type == "confirmed" || o.outcome_type == "prediction_correct")
                    .count();
                Some(format!("{}/{} outcomes confirmed", confirmed, outcomes.len()))
            }
            _ => None,
        };

        // Build confidence drivers
        let mut confidence_drivers: Vec<ConfidenceDriver> = Vec::new();
        let evidence_count = supporting_evidence.len();
        if evidence_count >= 3 {
            confidence_drivers.push(ConfidenceDriver {
                factor: "evidence".into(),
                impact: format!("{} independent sources", evidence_count),
            });
        } else if evidence_count >= 1 {
            confidence_drivers
                .push(ConfidenceDriver { factor: "evidence".into(), impact: format!("{} source", evidence_count) });
        }
        if trend == "increasing" {
            confidence_drivers
                .push(ConfidenceDriver { factor: "trend".into(), impact: "Confidence rising over time".into() });
        }
        if let Some(ref _hypothesis) = decision.hypothesis {
            confidence_drivers.push(ConfidenceDriver {
                factor: "analysis".into(),
                impact: "Based on structured hypothesis testing".into(),
            });
        }

        // Uncertainties
        let mut uncertainties: Vec<String> = Vec::new();
        if evidence_count < 5 {
            uncertainties.push("Limited number of supporting sources".into());
        }
        if outcome_summary.is_none() {
            uncertainties.push("Outcome not yet observed — prediction pending".into());
        }

        // Load reasoning framework traces
        let frameworks_applied: Vec<FrameworkTrace> = self
            .store
            .get_decision_framework_traces(id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|row| FrameworkTrace {
                id: row["framework_id"].as_str().unwrap_or("").to_string(),
                name: row["name"].as_str().unwrap_or("").to_string(),
                category: row["category"].as_str().unwrap_or("").to_string(),
                relevance: row["relevance"].as_f64().unwrap_or(0.0),
                reasoning: row["reasoning"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(Some(DecisionExplanation {
            decision_id: format!("DEC-{:06}", id),
            decision_title: decision.title,
            hypothesis: decision.hypothesis,
            confidence: decision.confidence,
            trend,
            supporting_evidence,
            confidence_drivers,
            uncertainties,
            outcome_summary,
            frameworks_applied,
        }))
    }

    // ── decision records ──

    /// List decision records (optionally by status) — GET `/api/decision-records`.
    pub async fn list_records(&self, status: Option<&str>, limit: u32) -> Result<Vec<DecisionRecord>, StoreError> {
        self.store.list_decision_records(status, limit).await
    }

    /// Single decision record; `Ok(None)` when missing — GET `decision-records/:id`.
    pub async fn record(&self, id: i64) -> Result<Option<DecisionRecord>, StoreError> {
        self.store.get_decision_record(id).await
    }

    /// Record + its outcome metrics and linked claims — GET `decision-records/:id`.
    /// `Ok(None)` when the record is missing; outcome/claim sub-reads are soft
    /// and degrade to empty lists, as the previous route behaviour did.
    pub async fn record_detail(&self, id: i64) -> Result<Option<DecisionRecordDetail>, StoreError> {
        let record = match self.store.get_decision_record(id).await {
            Ok(Some(r)) => r,
            Ok(None) => return Ok(None),
            Err(e) => return Err(e),
        };
        let outcomes = self.store.list_decision_outcomes(id).await.unwrap_or_default();
        let claims = self.store.get_decision_claims(id).await.unwrap_or_default();
        Ok(Some(DecisionRecordDetail { record, outcomes, claims }))
    }

    /// Outcome metrics for a decision record — GET `decision-records/:id/outcomes`.
    pub async fn record_outcomes(&self, decision_id: i64) -> Result<Vec<DecisionOutcome>, StoreError> {
        self.store.list_decision_outcomes(decision_id).await
    }

    /// Reasoning-framework traces applied to a decision's claims (loose rows).
    pub async fn framework_traces(&self, decision_id: i64) -> Result<Vec<serde_json::Value>, StoreError> {
        self.store.get_decision_framework_traces(decision_id).await
    }

    /// Persist a generated memo JSON against a decision record.
    pub async fn save_memo(&self, id: i64, memo_json: &str) -> Result<(), StoreError> {
        self.store.set_decision_memo(id, memo_json).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    // MemoryStore models decisions/outcomes/evaluations/confidence in-memory but
    // stubs the decision-record rows (`DecisionRecordStore` → "not implemented"),
    // so these tests pin the decision read-model contract that the stub can
    // express: missing rows surface as `Ok(None)`, list reads as empty, and the
    // memo/framework paths propagate the stub error.  No MemoryStore behaviour
    // is expanded here.

    #[test]
    fn detail_missing_decision_is_none() {
        let svc = DecisionReadService::new(MemoryStore::new());
        let found = futures::executor::block_on(svc.detail(999)).expect("detail should succeed");
        assert!(found.is_none());
    }

    #[test]
    fn list_empty_from_stub_backend() {
        let svc = DecisionReadService::new(MemoryStore::new());
        let decisions = futures::executor::block_on(svc.list(None, 50)).expect("list should succeed");
        assert!(decisions.is_empty());
    }

    #[test]
    fn stats_zero_from_stub_backend() {
        let svc = DecisionReadService::new(MemoryStore::new());
        let stats = futures::executor::block_on(svc.stats()).expect("stats should succeed");
        assert_eq!(stats.total_decisions, 0);
    }

    #[test]
    fn timeline_missing_decision_is_none() {
        let svc = DecisionReadService::new(MemoryStore::new());
        let tl = futures::executor::block_on(svc.timeline(999)).expect("timeline should succeed");
        assert!(tl.is_none());
    }

    #[test]
    fn explanation_missing_decision_is_none() {
        let svc = DecisionReadService::new(MemoryStore::new());
        let expl = futures::executor::block_on(svc.explanation(999)).expect("explanation should succeed");
        assert!(expl.is_none());
    }

    #[test]
    fn record_paths_error_on_unimplemented_stub() {
        let svc = DecisionReadService::new(MemoryStore::new());
        assert!(futures::executor::block_on(svc.record(1)).is_err());
        assert!(futures::executor::block_on(svc.record_outcomes(1)).is_err());
        assert!(futures::executor::block_on(svc.framework_traces(1)).is_err());
    }
}
