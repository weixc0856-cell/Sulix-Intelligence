//! Domain-owned repository port for the Reflection aggregate.
//!
//! Defined here (not in `store`) so the reflection engine depends on no
//! infrastructure. The concrete `D1ReflectionRepository` lives in
//! `crates/infrastructure`, which maps between these domain records and the D1
//! rows.
//!
//! Method signatures express the capabilities the engine needs (start a run,
//! transition its lifecycle, read back the decision context it reflects on,
//! enqueue an outbound event) — not the shape of a D1 table.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::context::{EvaluationSnapshot, OutcomeSnapshot};
use crate::error::ReflectionError;

/// A reflection persistence record — the row state the engine reads back.
///
/// Only the fields the engine consumes on read are modelled here; the adapter
/// decides how those map to D1 columns.
#[derive(Debug, Clone)]
pub struct ReflectionRecord {
    pub id: i64,
    pub decision_id: i64,
    pub retry_count: i64,
}

/// A partial update applied to a reflection's lifecycle state.
///
/// The engine drives status transitions with varying payloads (a lease carry
/// `started_at`/`lease_until`, a successful generation carries the result +
/// artifact, a failure carries `retry_count` + `last_error`), so the update is
/// a partial patch rather than a handful of bespoke methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionUpdate {
    pub id: i64,
    pub status: String,
    pub result: Option<String>,
    pub quality_score: Option<f64>,
    pub artifact_key: Option<String>,
    pub lessons_count: Option<i64>,
    pub rules_count: Option<i64>,
    pub retry_count: Option<i64>,
    pub last_error: Option<String>,
    pub started_at: Option<i64>,
    pub lease_until: Option<i64>,
}

/// Facts about the decision a reflection is generated for, assembled by the
/// adapter from D1 rows into domain value objects.
///
/// `None` from [`ReflectionRepository::load_decision_context`] means the
/// decision does not exist.
#[derive(Debug, Clone)]
pub struct DecisionFacts {
    pub decision_id: i64,
    pub title: String,
    pub decision_type: String,
    pub hypothesis: Option<String>,
    pub confidence: f64,
    pub outcome: Option<OutcomeSnapshot>,
    pub evaluations: Vec<EvaluationSnapshot>,
}

/// Repository for Reflection aggregate persistence.
#[async_trait(?Send)]
pub trait ReflectionRepository {
    /// Start a reflection run for a decision: create the reflection row in its
    /// initial (`generating`) state and return its id.
    async fn create(&self, decision_id: i64, job_id: &str) -> Result<i64, ReflectionError>;

    /// Persist a lifecycle transition (partial update).
    async fn update(&self, update: &ReflectionUpdate) -> Result<(), ReflectionError>;

    /// Most recent reflection row for a decision (used for retry bookkeeping).
    async fn find_latest_for_decision(&self, decision_id: i64) -> Result<Option<ReflectionRecord>, ReflectionError>;

    /// Load the decision facts the engine needs to generate a reflection.
    async fn load_decision_context(&self, decision_id: i64) -> Result<Option<DecisionFacts>, ReflectionError>;

    /// Enqueue an outbound event payload (event outbox → R2 archive via cron).
    async fn enqueue_event(
        &self,
        object_type: &str,
        object_key: &str,
        payload: &serde_json::Value,
    ) -> Result<(), ReflectionError>;
}
