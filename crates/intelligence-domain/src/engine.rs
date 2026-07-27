//! IntelligenceEngine — the single entry point for the Intelligence bounded context.
//!
//! This is a **deep module**: external code calls only `observe()` and `analyze()`.
//! The internal pipeline (extract → validate → confidence → signal) is hidden.

use crate::claim::Claim;
use crate::error::IntelligenceError;
use crate::observation::NewObservation;
use crate::repositories::{ClaimRepository, ObservationRepository, SignalRepository};

/// Engine for the Intelligence bounded context.
///
/// Generic over repository implementations (D1 for production, Memory for tests).
pub struct IntelligenceEngine<R, C, S>
where
    R: ObservationRepository,
    C: ClaimRepository,
    S: SignalRepository,
{
    observations: R,
    claims: C,
    signals: S,
}

impl<R, C, S> IntelligenceEngine<R, C, S>
where
    R: ObservationRepository,
    C: ClaimRepository,
    S: SignalRepository,
{
    /// Create a new engine with repository implementations.
    pub fn new(observations: R, claims: C, signals: S) -> Self {
        Self { observations, claims, signals }
    }

    /// Observe content from an external source.
    ///
    /// Creates an observation record and returns its ID.
    pub async fn observe(&self, input: NewObservation) -> Result<i64, IntelligenceError> {
        if input.title.is_empty() {
            return Err(IntelligenceError::InvalidInput("observation title must not be empty".into()));
        }
        self.observations.save(&input).await
    }

    /// Analyze an observation to extract claims.
    ///
    /// Creates claim records and returns them.
    pub async fn analyze(&self, _observation_id: i64, _article_id: i64) -> Result<Vec<Claim>, IntelligenceError> {
        // TODO: wire LLM-based claim extraction (claim-engine)
        // For now, placeholder that returns empty results
        Ok(Vec::new())
    }

    /// Detect signals from existing claims.
    ///
    /// Creates or updates signal threads.
    pub async fn detect_signals(&self, _score: f64) -> Result<i64, IntelligenceError> {
        // TODO: wire signal detection from claim patterns (signal-engine)
        Err(IntelligenceError::NotFound("signal detection not yet wired".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // In-memory repositories for testing
    use crate::claim::{Claim, NewClaim};
    use crate::observation::{NewObservation, Observation};
    use crate::signal::{NewSignalThread, SignalThread};
    use async_trait::async_trait;
    use std::cell::RefCell;

    struct MemObservationRepo {
        observations: RefCell<Vec<NewObservation>>,
        next_id: RefCell<i64>,
    }

    impl MemObservationRepo {
        fn new() -> Self {
            Self { observations: RefCell::new(Vec::new()), next_id: RefCell::new(1) }
        }
    }

    #[async_trait(?Send)]
    impl ObservationRepository for MemObservationRepo {
        async fn save(&self, observation: &NewObservation) -> Result<i64, IntelligenceError> {
            let id = *self.next_id.borrow();
            *self.next_id.borrow_mut() = id + 1;
            self.observations.borrow_mut().push(NewObservation {
                source_type: observation.source_type.clone(),
                source_id: observation.source_id.clone(),
                title: observation.title.clone(),
                summary: observation.summary.clone(),
                url: observation.url.clone(),
            });
            Ok(id)
        }
        async fn find(&self, _id: i64) -> Result<Option<Observation>, IntelligenceError> {
            Ok(None)
        }
        async fn find_by_hash(&self, _hash: &str) -> Result<Option<Observation>, IntelligenceError> {
            Ok(None)
        }
        async fn list(&self, _source_type: Option<&str>, _limit: u32) -> Result<Vec<Observation>, IntelligenceError> {
            Ok(Vec::new())
        }
    }

    struct MemClaimRepo;
    #[async_trait(?Send)]
    impl ClaimRepository for MemClaimRepo {
        async fn save(&self, _claim: &NewClaim) -> Result<i64, IntelligenceError> {
            Ok(1)
        }
        async fn find(&self, _id: i64) -> Result<Option<Claim>, IntelligenceError> {
            Ok(None)
        }
        async fn list(&self, _status: Option<&str>, _limit: u32) -> Result<Vec<Claim>, IntelligenceError> {
            Ok(Vec::new())
        }
    }

    struct MemSignalRepo;
    #[async_trait(?Send)]
    impl SignalRepository for MemSignalRepo {
        async fn upsert_thread(&self, _thread: &NewSignalThread) -> Result<i64, IntelligenceError> { Ok(1) }
        async fn find_thread(&self, _id: i64) -> Result<Option<SignalThread>, IntelligenceError> { Ok(None) }
        async fn append_instance(&self, _thread_id: i64, _score: f64, _impact: &str, _trend: &str) -> Result<i64, IntelligenceError> { Ok(1) }
        async fn update_lifecycle(&self, _now: i64) -> Result<(), IntelligenceError> { Ok(()) }
        async fn list_active(&self, _limit: u32) -> Result<Vec<SignalThread>, IntelligenceError> { Ok(Vec::new()) }
    }

    #[test]
    fn observe_creates_observation() {
        let engine = IntelligenceEngine::new(MemObservationRepo::new(), MemClaimRepo, MemSignalRepo);
        let input = NewObservation {
            source_type: "RssFeed".into(),
            source_id: "feed-1".into(),
            title: "Test article".into(),
            summary: None,
            url: None,
        };
        let id = futures::executor::block_on(engine.observe(input)).unwrap();
        assert_eq!(id, 1);
    }

    #[test]
    fn observe_rejects_empty_title() {
        let engine = IntelligenceEngine::new(MemObservationRepo::new(), MemClaimRepo, MemSignalRepo);
        let input = NewObservation {
            source_type: "RssFeed".into(),
            source_id: "feed-1".into(),
            title: "".into(),
            summary: None,
            url: None,
        };
        let result = futures::executor::block_on(engine.observe(input));
        assert!(result.is_err());
    }
}
