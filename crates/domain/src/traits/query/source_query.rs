use async_trait::async_trait;

use crate::{Source, StoreError};

/// Query service for reading source governance metadata.
#[async_trait(?Send)]
pub trait SourceQueryService {
    /// List all sources, optionally filtered by tier or policy.
    async fn list_sources(
        &self,
        tier: Option<&str>,
        policy: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Source>, StoreError>;
}
