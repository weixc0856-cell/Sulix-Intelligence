use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;

impl crate::D1Store {
    /// Persist a generated daily briefing. Uses INSERT OR REPLACE so
    /// re-generating the same date overwrites the previous version.
    pub async fn save_briefing(
        &self,
        date: &str,
        generated_at: i64,
        signal_count: u32,
        content: &str,
    ) -> Result<(), crate::StoreError> {
        self.db
            .prepare(
                "INSERT OR REPLACE INTO intelligence_briefs (date, generated_at, signal_count, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(&[
                date.into(),
                JsValue::from_f64(generated_at as f64),
                JsValue::from_f64(signal_count as f64),
                content.into(),
                JsValue::from_f64(generated_at as f64),
            ])?
            .run()
            .await?;
        Ok(())
    }

    /// Load today's briefing (the one whose `date` column matches today's
    /// YYYY-MM-DD string). Returns `None` if no briefing was generated yet.
    pub async fn load_today_briefing(&self, date: &str) -> Result<Option<String>, crate::StoreError> {
        let row: Option<serde_json::Value> = self
            .db
            .prepare("SELECT content FROM intelligence_briefs WHERE date = ?1")
            .bind(&[date.into()])?
            .first::<serde_json::Value>(None)
            .await?;
        Ok(row.and_then(|v| v["content"].as_str().map(String::from)))
    }

    /// List all available briefings, newest first.
    pub async fn list_briefings(&self) -> Result<Vec<BriefingSummary>, crate::StoreError> {
        #[derive(Deserialize)]
        struct Row {
            id: i64,
            date: String,
            generated_at: i64,
            signal_count: i64,
            created_at: i64,
        }
        let rows: Vec<Row> = self
            .db
            .prepare("SELECT id, date, generated_at, signal_count, created_at FROM intelligence_briefs ORDER BY date DESC LIMIT 90")
            .bind(&[])?
            .all()
            .await?
            .results()?;
        Ok(rows
            .into_iter()
            .map(|r| BriefingSummary {
                id: r.id,
                date: r.date,
                generated_at: r.generated_at,
                signal_count: r.signal_count as u32,
                created_at: r.created_at,
            })
            .collect())
    }

    /// Get a briefing by its database id.
    pub async fn get_briefing_by_id(&self, id: i64) -> Result<Option<String>, crate::StoreError> {
        let row: Option<serde_json::Value> = self
            .db
            .prepare("SELECT content FROM intelligence_briefs WHERE id = ?1")
            .bind(&[JsValue::from_f64(id as f64)])?
            .first::<serde_json::Value>(None)
            .await?;
        Ok(row.and_then(|v| v["content"].as_str().map(String::from)))
    }
}

/// Summary of a historical briefing for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingSummary {
    pub id: i64,
    pub date: String,
    pub generated_at: i64,
    pub signal_count: u32,
    pub created_at: i64,
}
