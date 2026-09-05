//! Rules application service — orchestrates the `/api/rules` CRUD use-cases.
//!
//! Generic over [`domain::RuleStore`].  The scoring pipeline reads enabled
//! rules through [`RuleStore::active_rule_jsons`], while the management
//! routes write through the CRUD methods.  Zero Worker / HTTP / `js_sys` code.
//!
//! The full-rule JSON reconstruction lives here, not in the HTTP handler: the
//! D1 `filter_rules.rule_json` column stores the complete scoring document
//! (`{name, audience_tag, condition, score_delta}`) that `rules::score`
//! parses, but the API accepts a condition-only JSON fragment.  Rewrapping is
//! therefore a use-case invariant, and a malformed fragment is surfaced as
//! [`RuleError::InvalidCondition`] for the route to map to a 400.

use domain::{SignalStrategy, StoreError};

/// Application service for the rule-management use-cases.
pub struct RuleService<S> {
    store: S,
}

/// Use-case outcomes that do not fit [`StoreError`].
///
/// Returned so the route layer can map each one onto the historical HTTP
/// contract without re-implementing the orchestration that produced it.
pub enum RuleError {
    /// `rule_json` did not parse as JSON (the route maps this to a 400).
    InvalidCondition(String),
    /// A referenced rule does not exist (the route maps this to a 404).
    NotFound,
    /// Persistence layer failure (the route maps this to a 500).
    Store(StoreError),
}

impl<S> RuleService<S>
where
    S: domain::RuleStore,
{
    /// Wrap a store (or store-backed repository) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// List all rules as raw JSON rows, newest first.
    pub async fn list(&self) -> Result<Vec<serde_json::Value>, StoreError> {
        self.store.list_rules().await
    }

    /// Get a single rule by id.
    pub async fn get(&self, id: i64) -> Result<Option<SignalStrategy>, StoreError> {
        self.store.get_rule(id).await
    }

    /// Create a rule.
    ///
    /// Parses `rule_json` as a condition fragment, rewraps it into the full
    /// scoring document, and inserts it.  Returns the new rule id; `None` is
    /// the defensive case where the store reported no id (a plain
    /// `INSERT … RETURNING` always yields one).
    pub async fn create(
        &self,
        name: &str,
        rule_json: &str,
        audience_tag: Option<&str>,
        signal_type: Option<&str>,
        score_delta: Option<f64>,
    ) -> Result<Option<i64>, RuleError> {
        let parsed_condition = serde_json::from_str::<serde_json::Value>(rule_json)
            .map_err(|e| RuleError::InvalidCondition(e.to_string()))?;
        let audience = audience_tag.unwrap_or("default");
        let score = score_delta.unwrap_or(0.0);

        let full_rule = serde_json::json!({
            "name": name,
            "audience_tag": audience,
            "condition": parsed_condition,
            "score_delta": score,
        });
        let full_rule_str = serde_json::to_string(&full_rule).unwrap_or_else(|_| rule_json.to_string());

        self.store.insert_rule(name, &full_rule_str, audience, signal_type, score).await.map_err(RuleError::Store)
    }

    /// Update a rule's editable fields.
    ///
    /// When `rule_json` (a condition fragment) is supplied, the existing rule
    /// is fetched first so the rewrapped full document keeps its stored
    /// `audience_tag` / `score_delta`; a rule that cannot be fetched for that
    /// rewrap is surfaced as [`RuleError::NotFound`] (mirroring the
    /// historical 404).  Returns the updated rule, or `None` when no row
    /// carries `id`.
    pub async fn update(
        &self,
        id: i64,
        name: Option<&str>,
        rule_json: Option<&str>,
        enabled: Option<bool>,
        signal_type: Option<Option<&str>>,
    ) -> Result<Option<SignalStrategy>, RuleError> {
        // Rewrap a condition-only fragment against the stored rule.  As in the
        // historical handler, an unparseable fragment becomes a null condition
        // rather than a 400, and any failure to fetch the existing rule (missing
        // row or store error) is a NotFound for this use-case.
        let mut rule_json_for_store: Option<String> = None;
        if let Some(cond_json) = rule_json {
            let existing = match self.store.get_rule(id).await {
                Ok(Some(rule)) => rule,
                _ => return Err(RuleError::NotFound),
            };
            let full_rule = serde_json::json!({
                "name": name.unwrap_or(&existing.name),
                "audience_tag": existing.audience_tag,
                "condition": serde_json::from_str::<serde_json::Value>(cond_json).unwrap_or_default(),
                "score_delta": existing.score_delta,
            });
            rule_json_for_store = Some(serde_json::to_string(&full_rule).unwrap_or_else(|_| cond_json.to_string()));
        }

        self.store
            .update_rule(id, name, rule_json_for_store.as_deref().or(rule_json), enabled, signal_type)
            .await
            .map_err(RuleError::Store)?;
        self.store.get_rule(id).await.map_err(RuleError::Store)
    }

    /// Soft-delete a rule by disabling it.
    pub async fn delete(&self, id: i64) -> Result<(), StoreError> {
        self.store.delete_rule(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    // MemoryStore does not model the rule CRUD rows (its `RuleStore` writes
    // return "not implemented"), so these tests pin the orchestration that
    // runs before the store is reached — fragment parsing / rewrapping and the
    // error mapping — rather than persistence round-trips.

    #[test]
    fn create_rejects_malformed_condition_json() {
        let svc = RuleService::new(MemoryStore::new());
        let result = futures::executor::block_on(svc.create("buy", "not json", None, None, None));
        assert!(matches!(result, Err(RuleError::InvalidCondition(_))));
    }

    #[test]
    fn create_propagates_store_failure() {
        let svc = RuleService::new(MemoryStore::new());
        let result = futures::executor::block_on(svc.create("buy", r#"{"op": "gte", "value": 0.8}"#, None, None, None));
        assert!(matches!(result, Err(RuleError::Store(_))));
    }

    #[test]
    fn update_rewrap_requires_existing_rule() {
        let svc = RuleService::new(MemoryStore::new());
        // Rewrapping a condition fragment needs the stored rule; when none can
        // be fetched the use-case is a NotFound, not a silent partial update.
        let result = futures::executor::block_on(svc.update(1, None, Some(r#"{"op": "lt"}"#), None, None));
        assert!(matches!(result, Err(RuleError::NotFound)));
    }
}
