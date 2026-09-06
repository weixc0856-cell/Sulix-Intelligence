use async_trait::async_trait;

use crate::{Memory, NewMemory, StoreError};

/// Memory-index persistence slice.
///
/// Named `MemoryPersistence` (not `MemoryRepository`) to avoid colliding with
/// the `MemoryStore` in-memory test double.
#[async_trait(?Send)]
pub trait MemoryPersistence {
    /// Create a memory row; returns the new memory id.
    async fn create_memory(&self, entry: &NewMemory) -> Result<i64, StoreError>;

    /// List memories, optionally filtered by memory_type + status.
    async fn list_memories(
        &self,
        memory_type: Option<&str>,
        status: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Memory>, StoreError>;
}
