use crate::{NewArtifact, ArtifactEntry, StoreError};
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
            ])?
            .first::<serde_json::Value>(None)
            .await?;

        row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| StoreError::D1("create_artifact failed: no id returned".into()))
    }

    /// List artifact_registry entries for a given entity.
    pub async fn list_artifacts_by_entity(&self, entity_id: i64, limit: u32) -> Result<Vec<ArtifactEntry>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT id, artifact_type, entity_id, r2_key, schema_version, model, pipeline_version, metadata, created_at \
                 FROM artifact_registry \
                 WHERE entity_id = ?1 \
                 ORDER BY created_at DESC \
                 LIMIT ?2",
            )
            .bind(&[JsValue::from_f64(entity_id as f64), JsValue::from_f64(limit as f64)])?
            .all()
            .await?
            .results()?)
    }
}
