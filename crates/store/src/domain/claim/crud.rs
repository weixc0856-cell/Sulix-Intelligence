use worker::wasm_bindgen::JsValue;

use crate::{ArticleEvidence, Claim, ClaimEvidence, NewClaim, StoreError};

impl crate::D1Store {
    /// Create a new claim (immutable — no confidence field).
    pub async fn create_claim(&self, c: &NewClaim) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO claims (statement, claim_type, reasoning, falsification, status, article_id, observation_id, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) RETURNING id",
            )
            .bind(&[
                c.statement.as_str().into(),
                c.claim_type.as_str().into(),
                c.reasoning.as_deref().map_or(JsValue::null(), |v| v.into()),
                c.falsification.as_deref().map_or(JsValue::null(), |v| v.into()),
                c.status.as_deref().unwrap_or("active").into(),
                c.article_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                c.observation_id.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
                JsValue::from_f64(now as f64),
                JsValue::from_f64(now as f64),
            ])?
            .first::<serde_json::Value>(None)
            .await?;
        row.and_then(|v| v["id"].as_i64()).ok_or_else(|| StoreError::D1("create_claim failed".into()))
    }

    /// Get a claim by id.
    pub async fn get_claim(&self, id: i64) -> Result<Option<Claim>, StoreError> {
        self.db
            .prepare(
                "SELECT id, statement, claim_type, reasoning, falsification, status, \
                 article_id, observation_id, created_at, updated_at \
                 FROM claims WHERE id = ?1",
            )
            .bind(&[JsValue::from_f64(id as f64)])?
            .first::<Claim>(None)
            .await
            .map_err(StoreError::from)
    }

    /// List claims with optional status and type filters.
    pub async fn list_claims(
        &self,
        status: Option<&str>,
        claim_type: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Claim>, StoreError> {
        let mut sql = String::from(
            "SELECT id, statement, claim_type, reasoning, falsification, status, \
             article_id, observation_id, created_at, updated_at \
             FROM claims WHERE 1=1",
        );
        let mut params: Vec<JsValue> = Vec::new();
        let mut idx = 1;

        if let Some(s) = status {
            sql.push_str(&format!(" AND status = ?{idx}"));
            params.push(s.into());
            idx += 1;
        }
        if let Some(t) = claim_type {
            sql.push_str(&format!(" AND claim_type = ?{idx}"));
            params.push(t.into());
            idx += 1;
        }

        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT ?{idx}"));
        params.push(JsValue::from_f64(limit as f64));
        idx += 1;
        sql.push_str(&format!(" OFFSET ?{idx}"));
        params.push(JsValue::from_f64(offset as f64));

        Ok(self.db.prepare(&sql).bind(&params)?.all().await?.results()?)
    }

    /// List claims by article_id.
    pub async fn list_claims_by_article(&self, article_id: i64) -> Result<Vec<Claim>, StoreError> {
        self.db
            .prepare(
                "SELECT id, statement, claim_type, reasoning, falsification, status, \
                 article_id, observation_id, created_at, updated_at \
                 FROM claims WHERE article_id = ?1 ORDER BY created_at DESC",
            )
            .bind(&[JsValue::from_f64(article_id as f64)])?
            .all()
            .await?
            .results()
            .map_err(StoreError::from)
    }

    /// Attach evidence to a claim.
    pub async fn attach_evidence(&self, e: &ClaimEvidence) -> Result<(), StoreError> {
        self.db
            .prepare(
                "INSERT OR REPLACE INTO claim_evidence (claim_id, article_id, relation, strength, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(&[
                JsValue::from_f64(e.claim_id as f64),
                JsValue::from_f64(e.article_id as f64),
                e.relation.as_str().into(),
                JsValue::from_f64(e.strength),
                JsValue::from_f64(e.created_at as f64),
            ])?
            .run()
            .await?;
        Ok(())
    }

    /// Get raw evidence rows for a claim (backward compat).
    pub async fn get_claim_evidence(&self, claim_id: i64) -> Result<Vec<ClaimEvidence>, StoreError> {
        Ok(self
            .db
            .prepare("SELECT claim_id, article_id, relation, strength, created_at FROM claim_evidence WHERE claim_id = ?1 ORDER BY strength DESC")
            .bind(&[JsValue::from_f64(claim_id as f64)])?
            .all()
            .await?
            .results()?)
    }

    /// Get evidence for a claim with article titles joined.
    pub async fn get_claim_evidence_with_articles(&self, claim_id: i64) -> Result<Vec<ArticleEvidence>, StoreError> {
        self.db
            .prepare(
                "SELECT ce.claim_id, ce.article_id, a.title AS article_title, \
                 ce.relation, ce.strength, ce.created_at \
                 FROM claim_evidence ce \
                 LEFT JOIN articles a ON a.id = ce.article_id \
                 WHERE ce.claim_id = ?1 ORDER BY ce.strength DESC",
            )
            .bind(&[JsValue::from_f64(claim_id as f64)])?
            .all()
            .await?
            .results()
            .map_err(StoreError::from)
    }
}
