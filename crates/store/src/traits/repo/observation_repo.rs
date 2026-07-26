use async_trait::async_trait;

use crate::{NewObservation, Observation, StoreError};

#[async_trait(?Send)]
pub trait ObservationRepository {
    async fn save_observation(&self, obs: &NewObservation) -> Result<i64, StoreError>;
    async fn find_observation(&self, id: i64) -> Result<Option<Observation>, StoreError>;
    async fn find_observation_by_hash(&self, hash: &str) -> Result<Option<Observation>, StoreError>;
}
