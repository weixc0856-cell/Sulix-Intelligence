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

    /// Failed-attempt cap beyond which a decision's reflection is given up on.
    /// Must stay in sync with the scheduling side (`store/d1/reflection/crud.rs`
    /// `failed_reflections_for_retry`: `retry_count < 3`); the adapter enforces
    /// the same bound on re-entry so a cap-exhausted row is never re-opened.
    const MAX_REFLECTION_RETRIES: i64 = 3;
}

#[async_trait(?Send)]
impl<S: ReflectionPersistence + DecisionRepository + OutcomeQueryService + EvaluationQueryService + OutboxStore>
    ReflectionRepository for D1ReflectionRepository<S>
{
    async fn create(&self, decision_id: i64, job_id: &str) -> Result<i64, ReflectionError> {
        // `reflections` is `UNIQUE(decision_id)` — at most one live row per
        // decision. A *retry* of a previously failed attempt must therefore
        // re-open the existing row rather than INSERT a second one (which would
        // violate the constraint and lock the decision out of reflection
        // forever after a single failure — R-1, 2026-09-06). Only a `failed` row
        // still under the retry cap is re-openable; a `generated`/`generating`
        // row (already reflected / currently in flight) or an exhausted `failed`
        // row is refused so a duplicate invocation can't disturb it.
        if let Some(existing) =
            self.store.get_reflection_by_decision(decision_id).await.map_err(Self::to_persistence)?
        {
            let open = existing.status == "failed" && existing.retry_count < Self::MAX_REFLECTION_RETRIES;
            if !open {
                return Err(ReflectionError::AlreadyTracked(decision_id));
            }
            // Re-open the failed row for a fresh attempt, preserving its
            // retry_count (the engine's `mark_failed` bumps it on the next
            // failure). update_reflection writes only Some fields + status.
            let reopen = store::UpdateReflection {
                id: existing.id,
                status: "generating".into(),
                result: None,
                quality_score: None,
                artifact_key: None,
                lessons_count: None,
                rules_count: None,
                retry_count: None,
                last_error: None,
                started_at: None,
                lease_until: None,
            };
            self.store.update_reflection(&reopen).await.map_err(Self::to_persistence)?;
            return Ok(existing.id);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use reflection_engine::error::ReflectionError;
    use store::memory::MemoryStore;

    type Repo = D1ReflectionRepository<MemoryStore>;

    fn repo() -> Repo {
        Repo::new(MemoryStore::new())
    }

    /// Mirror the engine's `mark_failed` write (failed + retry N) so a test can
    /// put a row into the exact state a failed attempt leaves behind.
    fn mark_failed(id: i64, retry_count: i64) -> ReflectionUpdate {
        ReflectionUpdate {
            id,
            status: "failed".into(),
            result: None,
            quality_score: None,
            artifact_key: None,
            lessons_count: None,
            rules_count: None,
            retry_count: Some(retry_count),
            last_error: Some("llm_error: boom".into()),
            started_at: None,
            lease_until: None,
        }
    }

    /// R-1 regression (persistence seam): `UNIQUE(decision_id)` means a retry of
    /// a failed attempt must re-open the existing row, not INSERT a second one.
    #[test]
    fn failed_row_under_cap_is_reopened_on_the_same_row() {
        let repo = repo();
        let first = futures::executor::block_on(repo.create(6, "job-a")).expect("fresh insert must succeed");
        futures::executor::block_on(repo.update(&mark_failed(first, 1))).unwrap();

        // Retry → the adapter re-opens the SAME row (this used to hit the
        // UNIQUE(decision_id) constraint on a second INSERT and deadlock the
        // decision out of reflection forever).
        let reopened = futures::executor::block_on(repo.create(6, "job-b")).expect("retry under cap must re-open");
        assert_eq!(reopened, first, "retry must return the existing row id, not a new one");

        // Re-open resets status to generating but PRESERVES the retry budget, so
        // the <3 cap still counts this attempt (a re-open that zeroed retry_count
        // would defeat the cap).
        let latest = futures::executor::block_on(repo.find_latest_for_decision(6)).unwrap().expect("row exists");
        assert_eq!(latest.id, first);
        assert_eq!(latest.retry_count, 1, "re-open must not reset the attempt budget");

        // The row is generating (in flight) again → a concurrent duplicate
        // invocation is refused, not clobbered.
        let dup = futures::executor::block_on(repo.create(6, "job-c"));
        assert!(matches!(dup, Err(ReflectionError::AlreadyTracked(6))));
    }

    #[test]
    fn generated_or_generating_row_is_refused_not_overwritten() {
        let repo = repo();
        let id = futures::executor::block_on(repo.create(5, "job-a")).unwrap();
        // Still generating (first create leaves it generating) → duplicate refused.
        assert!(matches!(
            futures::executor::block_on(repo.create(5, "job-b")),
            Err(ReflectionError::AlreadyTracked(5))
        ));
        // Move it to generated (a completed reflection) → still refused.
        futures::executor::block_on(repo.update(&ReflectionUpdate {
            id,
            status: "generated".into(),
            result: Some("adoption-timeline assumption failed".into()),
            quality_score: Some(0.9),
            artifact_key: None,
            lessons_count: None,
            rules_count: None,
            retry_count: None,
            last_error: None,
            started_at: None,
            lease_until: None,
        }))
        .unwrap();
        assert!(matches!(
            futures::executor::block_on(repo.create(5, "job-c")),
            Err(ReflectionError::AlreadyTracked(5))
        ));
        let latest = futures::executor::block_on(repo.find_latest_for_decision(5)).unwrap().unwrap();
        assert_eq!(latest.id, id, "refused create must not have touched the row");
    }

    /// Cap boundary: retry_count 2 (< 3) may still be re-opened; retry_count 3
    /// is given up on. This is what stops the failed-reflections retry list from
    /// re-picking a decision forever.
    #[test]
    fn failed_row_at_or_over_cap_is_given_up_not_reopened() {
        let repo = repo();
        // retry_count == 2 → one attempt left.
        let a = futures::executor::block_on(repo.create(7, "job-a")).unwrap();
        futures::executor::block_on(repo.update(&mark_failed(a, 2))).unwrap();
        assert_eq!(
            futures::executor::block_on(repo.create(7, "job-b")).unwrap(),
            a,
            "retry_count 2 is still under the cap"
        );
        // retry_count == 3 → exhausted, refused.
        let b = futures::executor::block_on(repo.create(8, "job-a")).unwrap();
        futures::executor::block_on(repo.update(&mark_failed(b, 3))).unwrap();
        assert!(matches!(
            futures::executor::block_on(repo.create(8, "job-b")),
            Err(ReflectionError::AlreadyTracked(8))
        ));
    }

    /// End-to-end at the persistence seam: fail → re-open → succeed leaves one
    /// surviving row carrying the original id, and a further create is refused
    /// once the row is generated.
    #[test]
    fn fail_reopen_then_generate_keeps_one_row_with_original_id() {
        let repo = repo();
        let first = futures::executor::block_on(repo.create(9, "job-a")).unwrap();
        futures::executor::block_on(repo.update(&mark_failed(first, 1))).unwrap();

        let reopened = futures::executor::block_on(repo.create(9, "job-b")).unwrap();
        assert_eq!(reopened, first);

        // Successful generation writes onto the SAME row (engine step 7 passes
        // retry_count: None so the budget is preserved, not zeroed).
        futures::executor::block_on(repo.update(&ReflectionUpdate {
            id: first,
            status: "generated".into(),
            result: Some("adoption-timeline assumption failed".into()),
            quality_score: Some(0.9),
            artifact_key: None,
            lessons_count: None,
            rules_count: None,
            retry_count: None,
            last_error: None,
            started_at: None,
            lease_until: None,
        }))
        .unwrap();

        let latest = futures::executor::block_on(repo.find_latest_for_decision(9)).unwrap().unwrap();
        assert_eq!(latest.id, first, "surviving row is the original, not a duplicate");
        assert_eq!(latest.retry_count, 1, "success keeps the attempt budget on the row");

        // Generated is final for this decision.
        assert!(matches!(
            futures::executor::block_on(repo.create(9, "job-c")),
            Err(ReflectionError::AlreadyTracked(9))
        ));
    }
}
