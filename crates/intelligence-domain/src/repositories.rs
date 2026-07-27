//! Domain-owned repository traits for the Intelligence bounded context.
//!
//! Defined here (not in `store`) so the domain depends on nothing
//! infrastructure-specific. Concrete implementations live in
//! `crates/infrastructure/d1/`.

use async_trait::async_trait;

use crate::claim::{Claim, NewClaim};
use crate::error::IntelligenceError;
use crate::observation::{NewObservation, Observation};
use crate::signal::{NewSignalThread, SignalThread};

/// Repository for Observation persistence.
#[async_trait(?Send)]
pub trait ObservationRepository {
    async fn save(&self, observation: &NewObservation) -> Result<i64, IntelligenceError>;
    async fn find(&self, id: i64) -> Result<Option<Observation>, IntelligenceError>;
    async fn find_by_hash(&self, hash: &str) -> Result<Option<Observation>, IntelligenceError>;
    async fn list(&self, source_type: Option<&str>, limit: u32) -> Result<Vec<Observation>, IntelligenceError>;
}

/// Repository for Claim persistence.
#[async_trait(?Send)]
pub trait ClaimRepository {
    async fn save(&self, claim: &NewClaim) -> Result<i64, IntelligenceError>;
    async fn find(&self, id: i64) -> Result<Option<Claim>, IntelligenceError>;
    async fn list(&self, status: Option<&str>, limit: u32) -> Result<Vec<Claim>, IntelligenceError>;
}

/// Repository for Signal thread persistence.
#[async_trait(?Send)]
pub trait SignalRepository {
    async fn upsert_thread(&self, thread: &NewSignalThread) -> Result<i64, IntelligenceError>;
    async fn find_thread(&self, id: i64) -> Result<Option<SignalThread>, IntelligenceError>;
    async fn append_instance(
        &self,
        thread_id: i64,
        score: f64,
        impact: &str,
        trend: &str,
    ) -> Result<i64, IntelligenceError>;
    async fn update_lifecycle(&self, now: i64) -> Result<(), IntelligenceError>;
    async fn list_active(&self, limit: u32) -> Result<Vec<SignalThread>, IntelligenceError>;
}
