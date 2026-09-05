use async_trait::async_trait;

use crate::{NewReflection, Reflection, StoreError, UpdateReflection};

/// Reflection-index persistence (Decision Reflection Engine).
///
/// Lifted off [`StoreBackend`](crate::StoreBackend) in P4 so infra adapters
/// bind this instead of the legacy supertrait. Named `ReflectionPersistence`
/// to avoid colliding with the `reflection_engine::repository::ReflectionRepository`
/// domain port it serves.
#[async_trait(?Send)]
pub trait ReflectionPersistence {
    /// Create a reflection row; returns the new reflection id.
    async fn create_reflection(&self, req: &NewReflection) -> Result<i64, StoreError>;

    /// Apply a partial update to an existing reflection.
    async fn update_reflection(&self, req: &UpdateReflection) -> Result<(), StoreError>;

    /// Load the latest reflection for a decision, if any.
    async fn get_reflection_by_decision(&self, decision_id: i64) -> Result<Option<Reflection>, StoreError>;
}
