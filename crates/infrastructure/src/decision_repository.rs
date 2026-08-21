//! D1-backed DecisionRepository — maps between Domain aggregate and D1 rows.
//!
//! Lives in infrastructure (not decision-engine) to keep domain pure.

use async_trait::async_trait;
use decision_engine::{DecisionAggregate, DecisionError, DecisionRepository, DecisionStatus, ReconstructDecision};
use shared_kernel::ids::DecisionId;
use store::StoreBackend;

/// Maps domain `DecisionAggregate` to/from D1 `decisions` table rows.
pub struct D1DecisionRepository<S> {
    store: S,
}

impl<S: StoreBackend> D1DecisionRepository<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    fn status_to_d1(status: &DecisionStatus) -> &'static str {
        match status {
            DecisionStatus::Draft => "draft",
            DecisionStatus::Proposed => "proposed",
            DecisionStatus::Approved => "approved",
            DecisionStatus::Executing => "active",
            DecisionStatus::Completed => "completed",
            DecisionStatus::Invalidated => "superseded",
        }
    }

    fn status_from_d1(s: &str) -> DecisionStatus {
        match s {
            "draft" => DecisionStatus::Draft,
            "proposed" => DecisionStatus::Proposed,
            "approved" => DecisionStatus::Approved,
            "active" => DecisionStatus::Executing,
            "completed" => DecisionStatus::Completed,
            "superseded" | "invalidated" => DecisionStatus::Invalidated,
            _ => DecisionStatus::Draft,
        }
    }

    fn d1_id(id: &str) -> Result<i64, DecisionError> {
        id.strip_prefix("DEC-")
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or_else(|| DecisionError::NotFound(id.to_string()))
    }

    fn from_store(d: store::Decision) -> DecisionAggregate {
        let status = Self::status_from_d1(&d.status);
        DecisionAggregate::reconstruct(ReconstructDecision {
            id: DecisionId::new(d.id),
            title: d.title,
            hypothesis: d.hypothesis,
            confidence: d.confidence,
            status,
            rationale: d.rationale,
            decision_type: d.decision_type,
            priority: d.priority,
            signal_thread_id: d.signal_thread_id,
            actor_id: d.actor_id,
            expected_outcomes: vec![], // expected_outcomes not in legacy decisions table
            observed_outcomes: vec![], // observed_outcomes not in legacy decisions table
            created_at: d.created_at,
            updated_at: d.updated_at,
        })
    }

    fn into_new(decision: &DecisionAggregate) -> store::NewDecision {
        store::NewDecision {
            signal_thread_id: None,
            actor_id: None,
            decision_type: decision.decision_type().to_string(),
            title: decision.title().to_string(),
            hypothesis: decision.hypothesis().map(String::from),
            rationale: decision.rationale().map(String::from),
            confidence: decision.confidence(),
            priority: decision.priority().to_string(),
        }
    }
}

#[async_trait(?Send)]
impl<S: StoreBackend> DecisionRepository for D1DecisionRepository<S> {
    async fn save(&self, decision: &DecisionAggregate) -> Result<(), DecisionError> {
        let new = Self::into_new(decision);
        self.store.create_decision(&new).await.map_err(|e| DecisionError::Infrastructure(e.to_string()))?;

        if let Ok(d1_id) = Self::d1_id(&decision.id().0) {
            let status = Self::status_to_d1(decision.status());
            self.store
                .update_decision_status(d1_id, status)
                .await
                .map_err(|e| DecisionError::Infrastructure(e.to_string()))?;
        }
        Ok(())
    }

    async fn find(&self, id: &str) -> Result<Option<DecisionAggregate>, DecisionError> {
        let d1_id = Self::d1_id(id)?;
        self.store
            .get_decision(d1_id)
            .await
            .map_err(|e| DecisionError::Infrastructure(e.to_string()))
            .map(|opt| opt.map(Self::from_store))
    }

    async fn find_by_signal(&self, signal_thread_id: i64) -> Result<Vec<DecisionAggregate>, DecisionError> {
        self.store
            .decisions_by_signal(signal_thread_id)
            .await
            .map_err(|e| DecisionError::Infrastructure(e.to_string()))
            .map(|vec| vec.into_iter().map(Self::from_store).collect())
    }

    async fn list(&self, status: Option<&str>, limit: u32) -> Result<Vec<DecisionAggregate>, DecisionError> {
        self.store
            .list_decisions(status, limit)
            .await
            .map_err(|e| DecisionError::Infrastructure(e.to_string()))
            .map(|vec| vec.into_iter().map(Self::from_store).collect())
    }
}

// Tests require wasm32 target (js_sys::Date in MemoryStore/StoreBackend).
// Run with: cargo test --target wasm32-unknown-unknown -p infrastructure
