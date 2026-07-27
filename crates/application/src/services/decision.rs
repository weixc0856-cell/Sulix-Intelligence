//! Decision Application Service — orchestrates the Decision aggregate with
//! outbox-first event emission.
//!
//! Generic over `S: StoreBackend` for backward compat during Sprint 6.2
//! transition. Will be narrowed to `R: DecisionRepository + O: OutboxPublisher`
//! when infrastructure crate (6.2D) lands.

use store::StoreBackend;

use decision_engine::{DecisionAggregate, DecisionError, ProposeDecision};

/// Application service for the Decision aggregate.
pub struct DecisionService<S> {
    #[allow(dead_code)]
    store: S,
}

impl<S: StoreBackend> DecisionService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    fn now() -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
    }

    /// Propose a new decision.
    ///
    /// Creates the aggregate, validates invariants, persists, and emits
    /// domain events through the outbox.
    pub async fn propose(&self, cmd: ProposeDecision) -> Result<DecisionAggregate, DecisionError> {
        let now = Self::now();
        let mut decision = DecisionAggregate::propose(cmd, now)?;
        let _events = decision.drain_events();
        // TODO: persist via DecisionRepository + emit events via OutboxPublisher
        //       when infrastructure crate (6.2D) provides D1DecisionRepository.
        Ok(decision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_propose_constructs_aggregate() {
        let store = store::memory::MemoryStore::new();
        let svc = DecisionService::new(store);
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
        assert!(result.is_ok());
        let agg = result.unwrap();
        assert_eq!(agg.title(), "Test decision");
    }
}
