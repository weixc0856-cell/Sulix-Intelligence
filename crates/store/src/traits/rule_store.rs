use async_trait::async_trait;

use crate::StoreError;

/// Rule-configuration read seam (enabled `rule_json` rows).
///
/// Lifted off [`StoreBackend`](crate::StoreBackend) in P4 so the rule
/// configuration surface has its own boundary instead of riding the legacy
/// supertrait.  Workers that score articles fetch the enabled rules here and
/// parse them into `rules::Rule` values.
#[async_trait(?Send)]
pub trait RuleStore {
    /// Return `rule_json` strings for every enabled rule matching `audience_tag`.
    async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError>;
}
