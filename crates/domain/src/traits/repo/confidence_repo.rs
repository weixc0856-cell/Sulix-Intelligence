use async_trait::async_trait;

use crate::{ConfidenceEvent, NewConfidenceEvent, StoreError};

/// ConfidenceRepository — append-only。
/// 不修改历史记录，只追加新事件。
#[async_trait(?Send)]
pub trait ConfidenceRepository {
    /// 记录一条置信度变化事件。返回事件 id。
    async fn append_confidence(&self, event: &NewConfidenceEvent) -> Result<i64, StoreError>;
    /// 获取某实体的置信度历史，按时间升序。
    async fn list_confidence_history(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<ConfidenceEvent>, StoreError>;
}
