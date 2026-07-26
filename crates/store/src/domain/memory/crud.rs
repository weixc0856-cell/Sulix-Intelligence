//! Memory Engine — D1Store CRUD.
//! See design spec: docs/superpowers/specs/2026-07-26-memory-engine-design.md

use worker::wasm_bindgen::JsValue;

use crate::{Memory, NewMemory, StoreError};

impl crate::D1Store {
    /// Create a new memory entry. Returns the new id.
    pub async fn create_memory(&self, entry: &NewMemory) -> Result<i64, StoreError> {
        let row = self
            .db
            .prepare(
                "INSERT INTO memory_index \
                 (memory_type, memory_origin, statement, confidence, stability_score, \
                  memory_sources, artifact_key, status) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                 RETURNING id",
            )
            .bind(&[
                entry.memory_type.as_str().into(),
                entry.memory_origin.as_str().into(),
                entry.statement.as_str().into(),
                JsValue::from_f64(entry.confidence),
                entry.stability_score.map_or(JsValue::null(), JsValue::from_f64),
                entry.memory_sources.as_deref().map_or(JsValue::null(), |v| v.into()),
                entry.artifact_key.as_deref().map_or(JsValue::null(), |v| v.into()),
                entry.status.as_str().into(),
            ])?
            .first::<serde_json::Value>(None)
            .await?;
        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| StoreError::D1("create_memory failed: no id returned".into()))
    }

    /// Get a memory entry by id.
    pub async fn get_memory(&self, id: i64) -> Result<Option<Memory>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT id, memory_type, memory_origin, statement, confidence, stability_score, \
                        confidence_updated_at, memory_sources, artifact_key, status, usage_count, \
                        validation_count, promoted_at, deprecated_at, last_used_at, created_at \
                 FROM memory_index WHERE id = ?1",
            )
            .bind(&[JsValue::from_f64(id as f64)])?
            .first::<Memory>(None)
            .await?)
    }

    /// List memories, optionally filtered by type and status.
    pub async fn list_memories(
        &self,
        memory_type: Option<&str>,
        status: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Memory>, StoreError> {
        let mut sql = String::from(
            "SELECT id, memory_type, memory_origin, statement, confidence, stability_score, \
                    confidence_updated_at, memory_sources, artifact_key, status, usage_count, \
                    validation_count, promoted_at, deprecated_at, last_used_at, created_at \
             FROM memory_index",
        );
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<JsValue> = Vec::new();

        if let Some(t) = memory_type {
            conditions.push(format!("memory_type = ?{}", params.len() + 1));
            params.push(t.into());
        }
        if let Some(s) = status {
            conditions.push(format!("status = ?{}", params.len() + 1));
            params.push(s.into());
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY created_at DESC LIMIT ?");
        params.push(JsValue::from_f64(limit as f64));

        Ok(self.db.prepare(sql).bind(&params)?.all().await?.results()?)
    }

    /// Update memory usage stats (increment usage_count, set last_used_at).
    pub async fn touch_memory(&self, id: i64, now: i64) -> Result<(), StoreError> {
        self.db
            .prepare("UPDATE memory_index SET usage_count = usage_count + 1, last_used_at = ?1 WHERE id = ?2")
            .bind(&[JsValue::from_f64(now as f64), JsValue::from_f64(id as f64)])?
            .run()
            .await?;
        Ok(())
    }

    /// Count memories pending promotion.
    pub async fn count_candidate_memories(&self) -> Result<i64, StoreError> {
        let row = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM memory_index WHERE status = 'candidate'")
            .bind(&[])?
            .first::<serde_json::Value>(None)
            .await?;
        row.and_then(|v| v["cnt"].as_i64()).ok_or_else(|| StoreError::D1("count_candidate_memories failed".into()))
    }
}
