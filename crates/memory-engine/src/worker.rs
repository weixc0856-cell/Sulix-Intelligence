use store::StoreBackend;
use worker::console_log;
use crate::candidate::extract_candidates;
use crate::evaluator::{evaluate, EvaluationResult};
use crate::promotion::promote;

pub const DEFAULT_BATCH_SIZE: u32 = 50;
pub const KV_LAST_RUN: &str = "memory:last_run";

pub async fn process_pending<S: StoreBackend>(store: &S, cache: &worker::kv::KvStore, now: i64) {
    if let Ok(Some(val)) = cache.get(KV_LAST_RUN).text().await {
        if let Ok(ts) = val.trim().parse::<i64>() {
            if now - ts < 86400 { return; }
        }
    }

    let candidates = match extract_candidates(store, now - 86400 * 7, DEFAULT_BATCH_SIZE).await {
        Ok(c) => c,
        Err(e) => { console_log!("[memory] extract_candidates failed: {e}"); return; }
    };

    if candidates.is_empty() {
        let _ = cache.put(KV_LAST_RUN, now.to_string());
        return;
    }

    for candidate in &candidates {
        let result = evaluate(0.75, true, true, true, 0.5, 0.5, 0.5);
        match result {
            EvaluationResult::Promote { score } => {
                match promote(store, candidate, &score, "Consolidated memory from reflection").await {
                    Ok(id) => console_log!("[memory] MEM-{:06} promoted", id),
                    Err(e) => console_log!("[memory] promotion failed: {e}"),
                }
            }
            EvaluationResult::Review { .. } => {
                console_log!("[memory] candidate {} needs review", candidate.reflection_id);
            }
            EvaluationResult::Archive { reason } => {
                console_log!("[memory] candidate {} archived: {reason}", candidate.reflection_id);
            }
        }
    }

    if let Ok(pb) = cache.put(KV_LAST_RUN, now.to_string()) {
        let _ = pb.expiration_ttl(604800).execute().await;
    }
    console_log!("[memory] consolidated {} candidates", candidates.len());
}
