//! Domain-owned repository port for Memory persistence.
//!
//! Defined here (not in `store`) so the memory engine depends on no
//! infrastructure. The concrete `D1MemoryRepository` lives in
//! `crates/infrastructure`, which maps between these domain records and the D1
//! rows.
//!
//! Outbox writes (`enqueue_event`) are a seam kept on this port for now so the
//! engine stays store-free; the store backend doc schedules relocating the
//! outbox to shared/events in a later phase.

use async_trait::async_trait;

use crate::error::MemoryError;
use crate::model::{MemoryEventRef, NewMemory};

/// Repository for Memory aggregate persistence.
#[async_trait(?Send)]
pub trait MemoryRepository {
    /// Persist a newly promoted memory and return its id.
    async fn create_memory(&self, memory: &NewMemory) -> Result<i64, MemoryError>;

    /// Enqueue an outbound event payload (event outbox → R2 archive via cron).
    async fn enqueue_event(
        &self,
        object_type: &str,
        object_key: &str,
        payload: &serde_json::Value,
    ) -> Result<(), MemoryError>;

    /// List recent reflection events the engine may consolidate into memories.
    ///
    /// Rows are returned in event order up to `limit`; the engine applies its
    /// own recency window, mirroring the legacy extraction semantics.
    async fn list_reflection_events(&self, limit: u32) -> Result<Vec<MemoryEventRef>, MemoryError>;
}
