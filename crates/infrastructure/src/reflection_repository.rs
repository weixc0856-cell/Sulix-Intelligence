//! D1-backed ReflectionRepository — maps between Reflection domain records and
//! D1 rows.
//!
//! Lives in infrastructure (not reflection-engine) to keep the domain pure.

use async_trait::async_trait;
use reflection_engine::context::{EvaluationSnapshot, OutcomeSnapshot};
use reflection_engine::error::ReflectionError;
use reflection_engine::repository::{DecisionFacts, ReflectionRecord, ReflectionRepository, ReflectionUpdate};
use store::{DecisionRepository, EvaluationQueryService, OutboxStore, OutcomeQueryService, ReflectionPersistence};

/// Maps Reflection aggregate persistence to the D1 `reflections` table.
pub struct D1ReflectionRepository<S> {
    store: S,
}

impl<S: ReflectionPersistence + DecisionRepository + OutcomeQueryService + EvaluationQueryService + OutboxStore>
    D1ReflectionRepository<S>
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    fn to_persistence(e: store::StoreError) -> ReflectionError {
        ReflectionError::Persistence(e.to_string())
    }
}

#[async_trait(?Send)]
impl<S: ReflectionPersistence + DecisionRepository + OutcomeQueryService + EvaluationQueryService + OutboxStore>
    ReflectionRepository for D1ReflectionRepository<S>
{
    async fn create(&self, decision_id: i64, job_id: &str) -> Result<i64, ReflectionError> {
        let new = store::NewReflection {
            decision_id,
            outcome_id: None,
            job_id: Some(job_id.to_string()),
            status: "generating".into(),
        };
        self.store.create_reflection(&new).await.map_err(Self::to_persistence)
    }

    async fn update(&self, update: &ReflectionUpdate) -> Result<(), ReflectionError> {
        let req = store::UpdateReflection {
            id: update.id,
            status: update.status.clone(),
            result: update.result.clone(),
            quality_score: update.quality_score,
            artifact_key: update.artifact_key.clone(),
            lessons_count: update.lessons_count,
            rules_count: update.rules_count,
            retry_count: update.retry_count,
            last_error: update.last_error.clone(),
            started_at: update.started_at,
            lease_until: update.lease_until,
        };
        self.store.update_reflection(&req).await.map_err(Self::to_persistence)
    }

    async fn find_latest_for_decision(&self, decision_id: i64) -> Result<Option<ReflectionRecord>, ReflectionError> {
        let row = self.store.get_reflection_by_decision(decision_id).await.map_err(Self::to_persistence)?;
        Ok(row.map(|r| ReflectionRecord { id: r.id, decision_id: r.decision_id, retry_count: r.retry_count }))
    }

    async fn load_decision_context(&self, decision_id: i64) -> Result<Option<DecisionFacts>, ReflectionError> {
        let decision = self.store.find_decision(decision_id).await.map_err(Self::to_persistence)?;
        let Some(d) = decision else { return Ok(None) };
        let outcomes = self.store.list_outcomes(decision_id).await.map_err(Self::to_persistence)?;
        let evaluations = self.store.list_evaluations(decision_id).await.map_err(Self::to_persistence)?;

        Ok(Some(DecisionFacts {
            decision_id: d.id,
            title: d.title,
            decision_type: d.decision_type,
            hypothesis: d.hypothesis,
            confidence: d.confidence,
            outcome: outcomes.into_iter().next().map(|o| OutcomeSnapshot {
                id: o.id,
                outcome_type: o.outcome_type,
                observation: o.observation,
            }),
            evaluations: evaluations
                .into_iter()
                .map(|e| EvaluationSnapshot {
                    evaluation: e.evaluation.to_string(),
                    confidence: e.confidence,
                    reasoning: e.reasoning,
                })
                .collect(),
        }))
    }

    async fn enqueue_event(
        &self,
        object_type: &str,
        object_key: &str,
        payload: &serde_json::Value,
    ) -> Result<(), ReflectionError> {
        let entry = store::NewOutbox {
            object_type: object_type.to_string(),
            object_key: object_key.to_string(),
            payload: payload.to_string(),
        };
        self.store.insert_outbox(&entry).await.map_err(Self::to_persistence).map(|_| ())
    }
}
