use crate::{ContextSnapshot, NewContextSnapshot, StoreError};
use worker::wasm_bindgen::JsValue;

impl crate::D1Store {
    pub async fn save_context_snapshot(&self, snap: &NewContextSnapshot) -> Result<(), StoreError> {
        self.db
            .prepare("INSERT INTO context_snapshots (id, query, intent, domain, context_json, object_key, object_size, evidence_refs, confidence, user_scope) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)")
            .bind(&[
                snap.id.as_str().into(),
                snap.query.as_str().into(),
                snap.intent.as_str().into(),
                snap.domain.as_deref().map_or(JsValue::null(), |v| v.into()),
                snap.context_json.as_str().into(),
                snap.object_key.as_deref().map_or(JsValue::null(), |v| v.into()),
                snap.object_size.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                snap.evidence_refs.as_deref().map_or(JsValue::null(), |v| v.into()),
                JsValue::from_f64(snap.confidence),
                snap.user_scope.as_deref().map_or(JsValue::null(), |v| v.into()),
            ])?
            .run()
            .await?;
        Ok(())
    }

    pub async fn get_context_snapshot(&self, id: &str) -> Result<Option<ContextSnapshot>, StoreError> {
        Ok(self
            .db
            .prepare("SELECT id, query, intent, domain, engine_version, context_json, object_key, object_size, evidence_refs, confidence, user_scope, created_at FROM context_snapshots WHERE id = ?1")
            .bind(&[id.into()])?
            .first::<ContextSnapshot>(None)
            .await?)
    }
}
