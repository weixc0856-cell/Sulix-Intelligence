//! Briefing Context — batch read model for the briefing pipeline.
//!
//! Provides a single method that returns all Intelligence context for a set
//! of signal threads in one call. This avoids the N+1 problem of querying
//! decisions/evaluations per thread individually.

use crate::s_err::StoreResultExt;
use std::collections::HashMap;

use worker::wasm_bindgen::JsValue;

/// Entity context as returned by the store for the briefing pipeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignalEntityRef {
    pub name: String,
    pub entity_type: String,
    pub confidence: Option<f64>,
}

/// Decision context as returned by the store for the briefing pipeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignalDecisionRef {
    pub id: i64,
    pub title: String,
    pub status: String,
}

/// Full briefing context bundle for a set of signal threads.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SignalBriefingContextBundle {
    pub entity_map: HashMap<i64, Vec<SignalEntityRef>>,
    pub decision_map: HashMap<i64, Vec<SignalDecisionRef>>,
    pub evaluation_map: HashMap<i64, Option<String>>, // decision_id → latest evaluation result
}

impl crate::D1Store {
    /// Batch-load briefing context for multiple signal threads.
    ///
    /// Returns a Bundle with entities, decisions, and latest evaluations
    /// mapped by thread_id and decision_id respectively.
    pub async fn get_signal_briefing_context_bundle(
        &self,
        thread_ids: &[i64],
    ) -> Result<SignalBriefingContextBundle, crate::StoreError> {
        if thread_ids.is_empty() {
            return Ok(SignalBriefingContextBundle::default());
        }

        let placeholders = crate::in_placeholders(thread_ids.len());
        let binds: Vec<JsValue> = thread_ids.iter().map(|id| JsValue::from_f64(*id as f64)).collect();

        // 1. Batch load entities per thread
        let entity_sql = format!(
            "SELECT st.id AS thread_id, e.name, e.entity_type, er.confidence \
             FROM signal_threads st \
             JOIN entity_relations er ON er.source_entity_id = st.anchor_entity_id \
             JOIN entities e ON e.id = CASE WHEN er.source_entity_id = st.anchor_entity_id THEN er.target_entity_id ELSE er.source_entity_id END \
             WHERE st.id IN ({placeholders}) \
             ORDER BY er.confidence DESC LIMIT 50"
        );
        let entity_rows: Vec<EntityRow> =
            self.db.prepare(&entity_sql).bind(&binds).s_err()?.all().await.s_err()?.results().s_err()?;

        let mut entity_map: HashMap<i64, Vec<SignalEntityRef>> = HashMap::new();
        for row in entity_rows {
            entity_map.entry(row.thread_id).or_default().push(SignalEntityRef {
                name: row.name,
                entity_type: row.entity_type,
                confidence: row.confidence,
            });
        }

        // 2. Batch load decisions per thread
        let decision_sql = format!(
            "SELECT id, signal_thread_id, title, status \
             FROM decisions \
             WHERE signal_thread_id IN ({placeholders}) \
             ORDER BY created_at DESC"
        );
        let decision_rows: Vec<DecisionRow> =
            self.db.prepare(&decision_sql).bind(&binds).s_err()?.all().await.s_err()?.results().s_err()?;

        let mut decision_map: HashMap<i64, Vec<SignalDecisionRef>> = HashMap::new();
        let mut decision_ids: Vec<i64> = Vec::new();
        for row in &decision_rows {
            decision_map.entry(row.signal_thread_id).or_default().push(SignalDecisionRef {
                id: row.id,
                title: row.title.clone(),
                status: row.status.clone(),
            });
            decision_ids.push(row.id);
        }

        // 3. Batch load latest evaluations per decision
        let mut evaluation_map: HashMap<i64, Option<String>> = HashMap::new();
        if !decision_ids.is_empty() {
            let eval_placeholders = crate::in_placeholders(decision_ids.len());
            let eval_binds: Vec<JsValue> = decision_ids.iter().map(|id| JsValue::from_f64(*id as f64)).collect();
            let eval_sql = format!(
                "SELECT de.id, d.id AS decision_id, de.evaluation AS result \
                 FROM decision_evaluations de \
                 JOIN decisions d ON d.id = de.decision_id \
                 WHERE de.id IN (\
                   SELECT MAX(id) FROM decision_evaluations \
                   WHERE decision_id IN ({eval_placeholders}) \
                   GROUP BY decision_id \
                 )"
            );
            let eval_rows: Vec<EvalRow> =
                self.db.prepare(&eval_sql).bind(&eval_binds).s_err()?.all().await.s_err()?.results().s_err()?;
            for row in eval_rows {
                evaluation_map.insert(row.decision_id, Some(row.result));
            }
        }

        Ok(SignalBriefingContextBundle { entity_map, decision_map, evaluation_map })
    }
}

#[derive(serde::Deserialize)]
struct EntityRow {
    thread_id: i64,
    name: String,
    entity_type: String,
    confidence: Option<f64>,
}

#[derive(serde::Deserialize)]
struct DecisionRow {
    id: i64,
    signal_thread_id: i64,
    title: String,
    status: String,
}

#[derive(serde::Deserialize)]
struct EvalRow {
    decision_id: i64,
    result: String,
}
