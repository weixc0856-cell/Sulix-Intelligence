use async_trait::async_trait;

use crate::{SignalStrategy, StoreError};

/// Rule-configuration read/write seam.
///
/// The rule-configuration surface has its own boundary instead of riding a
/// store-wide composite.  Workers that score articles fetch the enabled rules
/// here and parse them into `rules::Rule` values.  The CRUD methods were added
/// in Phase 2 so the API's `/api/rules` use-cases run through this seam.
#[async_trait(?Send)]
pub trait RuleStore {
    /// Return `rule_json` strings for every enabled rule matching `audience_tag`.
    async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError>;

    /// List all rules as raw JSON rows (id / name / rule_json / …), newest
    /// first.
    async fn list_rules(&self) -> Result<Vec<serde_json::Value>, StoreError>;

    /// Get a single rule by id.
    async fn get_rule(&self, id: i64) -> Result<Option<SignalStrategy>, StoreError>;

    /// Insert a rule; returns the new rule id.
    async fn insert_rule(
        &self,
        name: &str,
        rule_json: &str,
        audience_tag: &str,
        signal_type: Option<&str>,
        score_delta: f64,
    ) -> Result<Option<i64>, StoreError>;

    /// Update a rule's editable fields; only the provided fields change.
    async fn update_rule(
        &self,
        id: i64,
        name: Option<&str>,
        rule_json: Option<&str>,
        enabled: Option<bool>,
        signal_type: Option<Option<&str>>,
    ) -> Result<(), StoreError>;

    /// Soft-delete a rule by disabling it.
    async fn delete_rule(&self, id: i64) -> Result<(), StoreError>;
}
