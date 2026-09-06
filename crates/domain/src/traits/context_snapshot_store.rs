use async_trait::async_trait;

use crate::{NewContextSnapshot, StoreError};

/// Context-snapshot persistence (Context Engine read-model snapshots).
///
/// Infra adapters bind this narrow seam directly.
#[async_trait(?Send)]
pub trait ContextSnapshotStore {
    /// Persist a context snapshot (evidence summary + user scope) to D1.
    async fn save_context_snapshot(&self, snap: &NewContextSnapshot) -> Result<(), StoreError>;
}
