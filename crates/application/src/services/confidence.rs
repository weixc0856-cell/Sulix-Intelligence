//! Confidence-history application service — read-only access to the append-only
//! 置信度演化追踪 for a single entity (decision / signal / claim).
//!
//! Generic over the narrowest store surface — [`domain::ConfidenceRepository`].
//! Contains zero Worker / HTTP / `js_sys` code; the HTTP layer parses the path
//! params and delegates the orchestration here.

use domain::{ConfidenceEvent, StoreError};

/// Application service for confidence-history read use-cases.
pub struct ConfidenceService<S> {
    store: S,
}

impl<S> ConfidenceService<S>
where
    S: domain::ConfidenceRepository,
{
    /// Wrap a store (or store-backed repository) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Fetch the confidence history for one entity, ordered oldest-first.
    pub async fn history(&self, entity_type: &str, entity_id: &str) -> Result<Vec<ConfidenceEvent>, StoreError> {
        self.store.list_confidence_history(entity_type, entity_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{ConfidenceRepository, NewConfidenceEvent};
    use store::memory::MemoryStore;

    fn seed(store: &MemoryStore, entity_type: &str, entity_id: &str, confidence: f64) {
        futures::executor::block_on(store.append_confidence(&NewConfidenceEvent {
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            confidence,
            reason: None,
            trigger_event: None,
            factors_json: None,
        }))
        .expect("append_confidence should succeed");
    }

    #[test]
    fn history_is_scoped_to_entity_and_links_previous() {
        let store = MemoryStore::new();
        seed(&store, "decision", "D1", 0.5);
        seed(&store, "decision", "D2", 0.9);
        seed(&store, "decision", "D1", 0.7);

        let svc = ConfidenceService::new(store);
        let hist = futures::executor::block_on(svc.history("decision", "D1")).expect("history should succeed");

        assert_eq!(hist.len(), 2);
        assert!(hist.iter().all(|e| e.entity_id == "D1"));
        // Second event records the previous confidence of the same entity.
        assert_eq!(hist[1].previous_confidence, Some(0.5));
        assert_eq!(hist[1].confidence, 0.7);
    }

    #[test]
    fn history_for_unknown_entity_is_empty() {
        let svc = ConfidenceService::new(MemoryStore::new());
        let hist = futures::executor::block_on(svc.history("claim", "missing")).expect("history should succeed");
        assert!(hist.is_empty());
    }

    #[test]
    fn history_distinguishes_entity_types() {
        let store = MemoryStore::new();
        seed(&store, "decision", "X", 0.6);
        seed(&store, "claim", "X", 0.4);

        let svc = ConfidenceService::new(store);
        let claims = futures::executor::block_on(svc.history("claim", "X")).expect("history should succeed");
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].confidence, 0.4);
    }
}
