use crate::{ContextSnapshot, NewContextSnapshot, StoreError};

impl crate::D1Store {
    pub async fn save_context_snapshot(&self, snap: &NewContextSnapshot) -> Result<(), StoreError> {
        self.db
            .prepare("INSERT INTO context_snapshots (id, query, intent, domain, context_json, evidence_refs, confidence, user_scope) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")
            .bind(&[
                snap.id.as_str().into(),
                snap.query.as_str().into(),
                snap.intent.as_str().into(),
                snap.domain.as_deref().map_or(worker::wasm_bindgen::JsValue::null(), |v| v.into()),
                snap.context_json.as_str().into(),
                snap.evidence_refs.as_deref().map_or(worker::wasm_bindgen::JsValue::null(), |v| v.into()),
                worker::wasm_bindgen::JsValue::from_f64(snap.confidence),
                snap.user_scope.as_deref().map_or(worker::wasm_bindgen::JsValue::null(), |v| v.into()),
            ])?
            .run()
            .await?;
        Ok(())
    }

    pub async fn get_context_snapshot(&self, id: &str) -> Result<Option<ContextSnapshot>, StoreError> {
        Ok(self
            .db
            .prepare("SELECT id, query, intent, domain, engine_version, context_json, evidence_refs, confidence, user_scope, created_at FROM context_snapshots WHERE id = ?1")
            .bind(&[id.into()])?
            .first::<ContextSnapshot>(None)
            .await?)
    }
}
