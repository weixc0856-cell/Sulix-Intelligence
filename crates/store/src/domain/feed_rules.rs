//! Rules CRUD — separate bounded context from Feed management.

use serde::Deserialize;
use serde_json::Value;
use worker::wasm_bindgen::JsValue;

use crate::{D1Store, SignalStrategy, SignalSummary, StoreError};

impl D1Store {
    pub async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError> {
        #[derive(Deserialize)]
        struct Row {
            rule_json: String,
        }
        let rows: Vec<Row> = self
            .db
            .prepare("SELECT rule_json FROM filter_rules WHERE audience_tag = ?1 AND enabled = 1")
            .bind(&[audience_tag.into()])?
            .all()
            .await?
            .results()?;
        Ok(rows.into_iter().map(|r| r.rule_json).collect())
    }

    pub async fn signal_summary(&self) -> Result<Vec<SignalSummary>, StoreError> {
        Ok(self.db.prepare("SELECT signal_type, COUNT(*) AS strategy_count, COALESCE(SUM(score_delta), 0) AS total_score_delta, COALESCE(AVG(score_delta), 0) AS avg_score_delta, SUM(CASE WHEN enabled = 1 THEN 1 ELSE 0 END) AS enabled_count FROM filter_rules GROUP BY signal_type ORDER BY total_score_delta DESC").all().await?.results()?)
    }

    pub async fn list_rules(&self) -> Result<Vec<Value>, StoreError> {
        Ok(self.db.prepare("SELECT id, name, signal_type, rule_json, audience_tag, score_delta, enabled, created_at, updated_at FROM filter_rules ORDER BY created_at DESC").all().await?.results()?)
    }

    pub async fn get_rule(&self, id: i64) -> Result<Option<SignalStrategy>, StoreError> {
        Ok(self.db.prepare("SELECT id, name, signal_type, rule_json, audience_tag, score_delta, enabled, created_at, updated_at FROM filter_rules WHERE id = ?1").bind(&[JsValue::from_f64(id as f64)])?.first::<SignalStrategy>(None).await?)
    }

    pub async fn insert_rule(
        &self,
        name: &str,
        rule_json: &str,
        audience_tag: &str,
        signal_type: Option<&str>,
        score_delta: f64,
    ) -> Result<Option<i64>, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self.db.prepare("INSERT INTO filter_rules (name, rule_json, audience_tag, signal_type, score_delta, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING id").bind(&[name.into(), rule_json.into(), audience_tag.into(), signal_type.map_or(JsValue::null(), |v| v.into()), JsValue::from_f64(score_delta), JsValue::from_f64(now as f64)])?.first::<serde_json::Value>(None).await?;
        Ok(row.and_then(|v| v["id"].as_i64()))
    }

    pub async fn update_rule(
        &self,
        id: i64,
        name: Option<&str>,
        rule_json: Option<&str>,
        enabled: Option<bool>,
        signal_type: Option<Option<&str>>,
    ) -> Result<(), StoreError> {
        let mut parts: Vec<String> = Vec::new();
        let mut vals: Vec<JsValue> = Vec::new();
        if let Some(v) = name {
            parts.push("name = ?".into());
            vals.push(v.into());
        }
        if let Some(v) = rule_json {
            parts.push("rule_json = ?".into());
            vals.push(v.into());
        }
        if let Some(v) = enabled {
            parts.push("enabled = ?".into());
            vals.push(JsValue::from_f64(if v { 1.0 } else { 0.0 }));
        }
        if let Some(ref v) = signal_type {
            parts.push("signal_type = ?".into());
            vals.push(v.map_or(JsValue::null(), |s| s.into()));
        }
        if parts.is_empty() {
            return Ok(());
        }
        parts.push("updated_at = ?".into());
        vals.push(JsValue::from_f64(js_sys::Date::now() / 1000.0));
        vals.push(JsValue::from_f64(id as f64));
        self.db
            .prepare(format!("UPDATE filter_rules SET {} WHERE id = ?", parts.join(", ")))
            .bind(&vals)?
            .run()
            .await?;
        Ok(())
    }

    pub async fn delete_rule(&self, id: i64) -> Result<(), StoreError> {
        self.db
            .prepare("UPDATE filter_rules SET enabled = 0, updated_at = ?1 WHERE id = ?2")
            .bind(&[JsValue::from_f64(js_sys::Date::now() / 1000.0), JsValue::from_f64(id as f64)])?
            .run()
            .await?;
        Ok(())
    }
}
