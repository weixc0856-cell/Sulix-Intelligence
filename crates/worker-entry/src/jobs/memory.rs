use store::D1Store;
use worker::*;

pub(crate) async fn process_pending(env: &Env, now: i64) {
    let store = match env.d1("DB") {
        Ok(db) => D1Store::new(db),
        Err(e) => { console_log!("[memory] D1 binding failed: {e}"); return; }
    };
    let cache = match env.kv("CACHE") {
        Ok(c) => c,
        Err(e) => { console_log!("[memory] KV binding failed: {e}"); return; }
    };
    memory_engine::worker::process_pending(&store, &cache, now).await;
}
