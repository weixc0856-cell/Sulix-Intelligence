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
}
