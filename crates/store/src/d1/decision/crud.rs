//! Decision CRUD — create, get, update decision records.

use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::{Decision, NewDecision};

impl crate::D1Store {
    /// Create a new decision record. Returns the new id.
    pub async fn create_decision(&self, d: &NewDecision) -> Result<i64, crate::StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO decisions \
                 (signal_thread_id, actor_id, decision_type, title, hypothesis, rationale, confidence, status, priority, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8, ?9, ?10) RETURNING id",
            )
            .bind(&[
                d.signal_thread_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                d.actor_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                d.decision_type.as_str().into(),
                d.title.as_str().into(),
                d.hypothesis.as_deref().map_or(JsValue::null(), |s| s.into()),
                d.rationale.as_deref().map_or(JsValue::null(), |s| s.into()),
                JsValue::from_f64(d.confidence),
                d.priority.as_str().into(),
                JsValue::from_f64(now as f64),
                JsValue::from_f64(now as f64),
            ]).s_err()?
            .first::<serde_json::Value>(None)
            .await.s_err()?;

        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| crate::StoreError::D1("create_decision failed".into()))
    }

    /// Allocate the next `decisions.id` for a new decision-engine aggregate.
    ///
    /// The row primary key is written explicitly from the aggregate id (see
    /// `upsert_decision`), so a fresh decision needs its id *before* the row
    /// exists. `MAX(id) + 1` keeps the aggregate id space aligned with the
    /// auto-increment space legacy rows used. Single-writer assumption — a
    /// concurrent create may hand out the same id (documented risk, see
    /// `domain::DecisionIdSource`).
    pub(crate) async fn next_decision_id(&self) -> Result<i64, crate::StoreError> {
        let row = self
            .db
            .prepare("SELECT COALESCE(MAX(id), 0) + 1 AS next FROM decisions")
            .bind(&[])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;
        Ok(row.and_then(|v| v["next"].as_i64()).unwrap_or(1))
    }

    /// Get a single decision by id.
    pub async fn get_decision(&self, id: i64) -> Result<Option<Decision>, crate::StoreError> {
        let result = self
            .db
            .prepare(
                "SELECT id, signal_thread_id, actor_id, decision_type, title, hypothesis, rationale, \
                        confidence, status, priority, expected_outcomes, created_at, updated_at \
                 FROM decisions WHERE id = ?1",
            )
            .bind(&[JsValue::from_f64(id as f64)])
            .s_err()?
            .first::<Decision>(None)
            .await
            .s_err()?;
        Ok(result)
    }

    /// Idempotent upsert of a full decision row (decision-engine vertical).
    ///
    /// The row's primary key (`decisions.id`) is written explicitly from the
    /// aggregate id, so `ON CONFLICT(id)` makes a second `save` of the same
    /// aggregate an in-place update rather than a duplicate insert. Update
    /// refreshes aggregate-owned columns and `updated_at` but **omits
    /// `created_at`** from the `DO UPDATE SET`, preserving the first insert's
    /// value (P2 field policy, 2026-09-06).
    pub(crate) async fn upsert_decision(&self, d: &Decision) -> Result<(), crate::StoreError> {
        self.db
            .prepare(
                "INSERT INTO decisions \
                 (id, signal_thread_id, actor_id, decision_type, title, hypothesis, rationale, confidence, status, \
                  priority, expected_outcomes, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) \
                 ON CONFLICT(id) DO UPDATE SET \
                   signal_thread_id = excluded.signal_thread_id, \
                   actor_id = excluded.actor_id, \
                   decision_type = excluded.decision_type, \
                   title = excluded.title, \
                   hypothesis = excluded.hypothesis, \
                   rationale = excluded.rationale, \
                   confidence = excluded.confidence, \
                   status = excluded.status, \
                   priority = excluded.priority, \
                   expected_outcomes = excluded.expected_outcomes, \
                   updated_at = excluded.updated_at",
            )
            .bind(&[
                JsValue::from_f64(d.id as f64),
                d.signal_thread_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                d.actor_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                d.decision_type.as_str().into(),
                d.title.as_str().into(),
                d.hypothesis.as_deref().map_or(JsValue::null(), |s| s.into()),
                d.rationale.as_deref().map_or(JsValue::null(), |s| s.into()),
                JsValue::from_f64(d.confidence),
                d.status.as_str().into(),
                d.priority.as_str().into(),
                d.expected_outcomes.as_deref().map_or(JsValue::null(), |s| s.into()),
                JsValue::from_f64(d.created_at as f64),
                JsValue::from_f64(d.updated_at as f64),
            ])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    /// Update decision status.
    pub async fn update_decision_status(&self, id: i64, status: &str) -> Result<(), crate::StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        self.db
            .prepare("UPDATE decisions SET status = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&[status.into(), JsValue::from_f64(now as f64), JsValue::from_f64(id as f64)])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }
}
