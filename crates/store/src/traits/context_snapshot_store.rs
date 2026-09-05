use async_trait::async_trait;

use crate::{NewContextSnapshot, StoreError};

/// Context-snapshot persistence (Context Engine read-model snapshots).
///
/// Lifted off [`StoreBackend`](crate::StoreBackend) in P4 so infra adapters
/// bind this instead of the legacy supertrait.
#[async_trait(?Send)]
pub trait ContextSnapshotStore {
    /// Persist a context snapshot (evidence summary + user scope) to D1.
    async fn save_context_snapshot(&self, snap: &NewContextSnapshot) -> Result<(), StoreError>;
}
