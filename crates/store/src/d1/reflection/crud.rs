//! Reflection Engine — D1Store CRUD.
//! See design spec: docs/superpowers/specs/2026-07-25-reflection-engine-design.md

use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::{NewReflection, Reflection, StoreError, UpdateReflection};

impl crate::D1Store {
    /// Create a new reflection row. Returns the new id.
    pub async fn create_reflection(&self, req: &NewReflection) -> Result<i64, StoreError> {
        let row = self
            .db
            .prepare(
                "INSERT INTO reflections (decision_id, outcome_id, job_id, status) \
                 VALUES (?1, ?2, ?3, ?4) RETURNING id",
            )
            .bind(&[
                JsValue::from_f64(req.decision_id as f64),
                req.outcome_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                req.job_id.as_deref().map_or(JsValue::null(), |v| v.into()),
                req.status.as_str().into(),
            ])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;
        row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| StoreError::D1("create_reflection failed: no id returned".into()))
    }

    /// Update reflection state (status, result, etc.).
    pub async fn update_reflection(&self, req: &UpdateReflection) -> Result<(), StoreError> {
        let mut parts: Vec<String> = vec!["status = ?1".into()];
        let mut vals: Vec<JsValue> = vec![req.status.as_str().into()];

        if let Some(v) = &req.result {
            parts.push("result = ?".into());
            vals.push(v.as_str().into());
        }
        if let Some(v) = req.quality_score {
            parts.push("quality_score = ?".into());
            vals.push(JsValue::from_f64(v));
        }
        if let Some(v) = &req.artifact_key {
            parts.push("artifact_key = ?".into());
            vals.push(v.as_str().into());
        }
        if let Some(v) = req.lessons_count {
            parts.push("lessons_count = ?".into());
            vals.push(JsValue::from_f64(v as f64));
        }
        if let Some(v) = req.rules_count {
            parts.push("rules_count = ?".into());
            vals.push(JsValue::from_f64(v as f64));
        }
        if let Some(v) = req.retry_count {
            parts.push("retry_count = ?".into());
            vals.push(JsValue::from_f64(v as f64));
        }
        if let Some(v) = &req.last_error {
            parts.push("last_error = ?".into());
            vals.push(v.as_str().into());
        }
        if let Some(v) = req.started_at {
            parts.push("started_at = ?".into());
            vals.push(JsValue::from_f64(v as f64));
        }
        if let Some(v) = req.lease_until {
            parts.push("lease_until = ?".into());
            vals.push(JsValue::from_f64(v as f64));
        }

        parts.push("updated_at = ?".into());
        vals.push(JsValue::from_f64(js_sys::Date::now() / 1000.0));

        vals.push(JsValue::from_f64(req.id as f64));

        self.db
            .prepare(format!("UPDATE reflections SET {} WHERE id = ?", parts.join(", ")))
            .bind(&vals)
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    /// Get a reflection by decision_id.
    pub async fn get_reflection_by_decision(&self, decision_id: i64) -> Result<Option<Reflection>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT id, decision_id, outcome_id, job_id, status, artifact_key, result, quality_score, \
                        generator_version, lessons_count, rules_count, generated_by, retry_count, last_error, \
                        started_at, lease_until, created_at, updated_at \
                 FROM reflections WHERE decision_id = ?1",
            )
            .bind(&[JsValue::from_f64(decision_id as f64)])
            .s_err()?
            .first::<Reflection>(None)
            .await
            .s_err()?)
    }

    /// List completed decisions (>7d) without a reflection.
    pub async fn decisions_eligible_for_reflection(&self, now: i64, limit: u32) -> Result<Vec<i64>, StoreError> {
        let cutoff = now - 604800;
        let rows: Vec<serde_json::Value> = self
            .db
            .prepare(
                "SELECT d.id FROM decisions d \
                 WHERE d.status IN ('completed', 'superseded') \
                   AND d.updated_at < ?1 \
                   AND NOT EXISTS (SELECT 1 FROM reflections r WHERE r.decision_id = d.id AND r.status != 'failed') \
                 LIMIT ?2",
            )
            .bind(&[JsValue::from_f64(cutoff as f64), JsValue::from_f64(limit as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()?;
        Ok(rows.iter().filter_map(|r| r["id"].as_i64()).collect())
    }

    /// List failed reflections eligible for retry (retry_count < 3).
    pub async fn failed_reflections_for_retry(&self, limit: u32) -> Result<Vec<Reflection>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT id, decision_id, outcome_id, job_id, status, artifact_key, result, quality_score, \
                        generator_version, lessons_count, rules_count, generated_by, retry_count, last_error, \
                        started_at, lease_until, created_at, updated_at \
                 FROM reflections \
                 WHERE status = 'failed' AND retry_count < 3 \
                 LIMIT ?1",
            )
            .bind(&[JsValue::from_f64(limit as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()?)
    }

    /// List stale generating reflections (lease expired).
    pub async fn stale_generating_reflections(&self, now: i64) -> Result<Vec<Reflection>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT id, decision_id, outcome_id, job_id, status, artifact_key, result, quality_score, \
                        generator_version, lessons_count, rules_count, generated_by, retry_count, last_error, \
                        started_at, lease_until, created_at, updated_at \
                 FROM reflections \
                 WHERE status = 'generating' AND lease_until < ?1 \
                 LIMIT 10",
            )
            .bind(&[JsValue::from_f64(now as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()?)
    }
}
