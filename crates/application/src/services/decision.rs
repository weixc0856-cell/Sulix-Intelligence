//! Decision Application Service — orchestrates the Decision aggregate with
//! outbox-first event emission and ArtifactRegistry-backed storage.
//!
//! Generic over:
//! - `S: StoreBackend` — persistence (narrowed in 6.2D)
//! - `A: ArtifactRegistry` — large-object storage

use shared_kernel::artifact_registry::{ArtifactRegistry, NewArtifact};
use store::StoreBackend;

use decision_engine::{DecisionAggregate, DecisionError, ProposeDecision};

/// Application service for the Decision aggregate.
pub struct DecisionService<S, A> {
    store: S,
    artifact_registry: A,
}

impl<S: StoreBackend, A: ArtifactRegistry> DecisionService<S, A> {
    pub fn new(store: S, artifact_registry: A) -> Self {
        Self { store, artifact_registry }
    }

    fn now() -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
    }

    /// Propose a new decision.
    ///
    /// Creates the aggregate, validates invariants, persists through store,
    /// and drains domain events (future: emit via OutboxPublisher).
    pub async fn propose(&self, cmd: ProposeDecision) -> Result<DecisionAggregate, DecisionError> {
        let now = Self::now();
        let mut decision = DecisionAggregate::propose(cmd, now)?;

        // Persist decision state via StoreBackend (legacy path)
        self.store
            .create_decision(&store::NewDecision {
                signal_thread_id: None,
                actor_id: None,
                decision_type: "experiment".into(),
                title: decision.title().into(),
                hypothesis: None,
                rationale: None,
                confidence: decision.confidence(),
                priority: "medium".into(),
            })
            .await
            .map_err(|e| DecisionError::Infrastructure(e.to_string()))?;

        // Drain events (future: emit via outbox)
        let _events = decision.drain_events();
        Ok(decision)
    }

    /// Store a decision memo through the ArtifactRegistry.
    ///
    /// This replaces inline `memo_json` storage in D1. The returned
    /// `artifact_id` should be stored on the decision record.
    pub async fn store_memo(
        &self,
        decision_id: &str,
        memo_content: &str,
        content_type: &str,
    ) -> Result<i64, DecisionError> {
        let artifact = NewArtifact {
            artifact_type: "decision_memo".into(),
            owner_type: "decision".into(),
            owner_id: decision_id.to_string(),
            content: memo_content.as_bytes().to_vec(),
            content_type: content_type.to_string(),
        };
        let artifact_ref =
            self.artifact_registry.store(artifact).await.map_err(|e| DecisionError::Infrastructure(e.to_string()))?;
        Ok(artifact_ref.artifact_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrastructure::artifact_registry::InMemoryRegistry;
    use store::memory::MemoryStore;

    #[test]
    fn service_propose_constructs_aggregate() {
        let store = MemoryStore::new();
        let registry = InMemoryRegistry;
        let svc = DecisionService::new(store, registry);
        let cmd = ProposeDecision {
            id: 1,
            title: "Test decision".into(),
            hypothesis: Some("X → Y".into()),
            confidence: 0.8,
            rationale: Some("Analysis".into()),
            decision_type: "experiment".into(),
            priority: "high".into(),
            signal_thread_id: None,
            actor_id: Some(1),
            expected_outcomes: vec![],
        };
        let result = futures::executor::block_on(svc.propose(cmd));
        assert!(result.is_ok(), "propose should succeed: {:?}", result.err());
        let agg = result.unwrap();
        assert_eq!(agg.title(), "Test decision");
    }
}
