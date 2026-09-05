use async_trait::async_trait;

use crate::{Observation, StoreError};

/// Query service for reading observation records.
#[async_trait(?Send)]
pub trait ObservationQueryService {
    /// List observations with optional filters.
    async fn list_observations(
        &self,
        source_type: Option<&str>,
        source_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Observation>, StoreError>;
}
