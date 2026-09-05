use crate::s_err::StoreResultExt;
use crate::{ArtifactEntry, ArtifactRecord, ArtifactRef, NewArtifact, NewArtifactRecord, NewArtifactRef, StoreError};
use worker::wasm_bindgen::JsValue;

impl crate::D1Store {
    /// Create an artifact_registry entry for an R2-stored intelligence asset.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_artifact(&self, artifact: &NewArtifact) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO artifact_registry \
                 (artifact_type, entity_id, r2_key, schema_version, model, pipeline_version, metadata, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) RETURNING id",
            )
            .bind(&[
                artifact.artifact_type.as_str().into(),
                JsValue::from_f64(artifact.entity_id as f64),
                artifact.r2_key.as_str().into(),
                artifact.schema_version.as_str().into(),
                artifact.model.as_deref().map_or(JsValue::null(), |v| v.into()),
                artifact.pipeline_version.as_str().into(),
                artifact.metadata.as_deref().map_or(JsValue::null(), |v| v.into()),
                JsValue::from_f64(now as f64),
            ])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;

        row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| StoreError::D1("create_artifact failed: no id returned".into()))
    }

    /// List artifact_registry entries for a given entity.
    pub async fn list_artifacts_by_entity(&self, entity_id: i64, limit: u32) -> Result<Vec<ArtifactEntry>, StoreError> {
        self
            .db
            .prepare(
                "SELECT id, artifact_type, entity_id, r2_key, schema_version, model, pipeline_version, metadata, created_at \
                 FROM artifact_registry \
                 WHERE entity_id = ?1 \
                 ORDER BY created_at DESC \
                 LIMIT ?2",
            )
            .bind(&[JsValue::from_f64(entity_id as f64), JsValue::from_f64(limit as f64)]).s_err()?
            .all()
            .await.s_err()?
            .results().s_err()
    }

    // ── Memory Artifacts (Sprint 5.1+, unified Memory Archive index) ──

    /// Register a new artifact in the memory_artifacts metadata index.
    pub async fn put_artifact(&self, artifact: &NewArtifactRecord) -> Result<i64, StoreError> {
        let row = self
            .db
            .prepare(
                "INSERT INTO memory_artifacts \
                 (artifact_type, artifact_date, object_key, schema_version, content_hash, size_bytes, metadata) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(artifact_type, artifact_date) DO UPDATE SET \
                   object_key = excluded.object_key, \
                   schema_version = excluded.schema_version, \
                   content_hash = excluded.content_hash, \
                   size_bytes = excluded.size_bytes, \
                   metadata = excluded.metadata \
                 RETURNING id",
            )
            .bind(&[
                artifact.artifact_type.as_str().into(),
                artifact.artifact_date.as_str().into(),
                artifact.object_key.as_str().into(),
                JsValue::from_f64(artifact.schema_version as f64),
                artifact.content_hash.as_deref().map_or(JsValue::null(), |v| v.into()),
                artifact.size_bytes.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                artifact.metadata.as_deref().map_or(JsValue::null(), |v| v.into()),
            ])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;
        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| StoreError::D1("put_artifact failed: no id returned".into()))
    }

    /// Retrieve an artifact record by type + date.
    pub async fn get_artifact(&self, artifact_type: &str, date: &str) -> Result<Option<ArtifactRecord>, StoreError> {
        self.db
            .prepare(
                "SELECT id, artifact_type, artifact_date, object_key, schema_version, \
                        content_hash, size_bytes, metadata, created_at \
                 FROM memory_artifacts \
                 WHERE artifact_type = ?1 AND artifact_date = ?2",
            )
            .bind(&[artifact_type.into(), date.into()])
            .s_err()?
            .first::<ArtifactRecord>(None)
            .await
            .s_err()
    }

    /// List artifacts of a given type, newest first.
    pub async fn list_artifacts(&self, artifact_type: &str, limit: u32) -> Result<Vec<ArtifactRecord>, StoreError> {
        self.db
            .prepare(
                "SELECT id, artifact_type, artifact_date, object_key, schema_version, \
                        content_hash, size_bytes, metadata, created_at \
                 FROM memory_artifacts \
                 WHERE artifact_type = ?1 \
                 ORDER BY artifact_date DESC LIMIT ?2",
            )
            .bind(&[artifact_type.into(), JsValue::from_f64(limit as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()
    }

    // ── Sprint 6.1 Artifact Registry (unified R2 object registry) ──

    pub async fn register_artifact_ref(&self, a: &NewArtifactRef) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO artifacts (artifact_type, artifact_key, content_type, size_bytes, hash, metadata, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING id",
            )
            .bind(&[
                a.artifact_type.as_str().into(),
                a.artifact_key.as_str().into(),
                a.content_type.as_str().into(),
                a.size_bytes.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                a.hash.as_deref().map_or(JsValue::null(), |v| v.into()),
                a.metadata.as_deref().map_or(JsValue::null(), |v| v.into()),
                JsValue::from_f64(now as f64),
            ]).s_err()?
            .first::<serde_json::Value>(None)
            .await.s_err()?;
        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| StoreError::D1("register_artifact_ref failed".into()))
    }

    pub async fn get_artifact_ref(&self, id: i64) -> Result<Option<ArtifactRef>, StoreError> {
        self.db
            .prepare("SELECT id, artifact_type, artifact_key, content_type, size_bytes, hash, version, metadata, created_at FROM artifacts WHERE id = ?1")
            .bind(&[JsValue::from_f64(id as f64)]).s_err()?
            .first::<ArtifactRef>(None)
            .await
            .s_err()
    }

    pub async fn find_artifact_ref(
        &self,
        artifact_type: &str,
        artifact_key: &str,
    ) -> Result<Option<ArtifactRef>, StoreError> {
        self.db
            .prepare("SELECT id, artifact_type, artifact_key, content_type, size_bytes, hash, version, metadata, created_at FROM artifacts WHERE artifact_type = ?1 AND artifact_key = ?2")
            .bind(&[artifact_type.into(), artifact_key.into()]).s_err()?
            .first::<ArtifactRef>(None)
            .await
            .s_err()
    }
}
