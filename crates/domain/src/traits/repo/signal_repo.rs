use async_trait::async_trait;

use crate::{SignalThread, StoreError};

/// Signal Thread aggregate persistence.
///
/// Manages the signal thread lifecycle (active / decaying / resolved / archived).
/// Signal instances (daily snapshots) and timeline events are written through
/// the `SignalStore` seam (`append_signal_instance_v2`, `insert_signal_event`)
/// until event sourcing is formalised.
/// Read-model queries (radar, detail, candidates) belong in
/// [`super::super::query::SignalQueryService`].
#[async_trait(?Send)]
pub trait SignalRepository {
    /// Insert or update a signal thread.  Returns the thread id.
    async fn save_signal(&self, thread: &SignalThread) -> Result<i64, StoreError>;

    /// Load a signal thread by its primary key.
    async fn find_signal(&self, id: i64) -> Result<Option<SignalThread>, StoreError>;

    /// Load a signal thread by its `signal_key` (e.g. `"entity:{entity_id}"`).
    async fn find_signal_by_key(&self, key: &str) -> Result<Option<SignalThread>, StoreError>;
}
