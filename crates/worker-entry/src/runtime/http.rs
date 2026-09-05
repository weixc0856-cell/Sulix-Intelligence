use worker::*;

use crate::jobs::ingestion;
use api::router;
use store::Store;

pub(crate) async fn handle(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    console_log!("[INFO] HTTP request: {} {}", req.method(), req.path());
    if req.path().to_lowercase().contains("__cron") {
        match ingestion::process_all_feeds(&env).await {
            Ok(_) => Response::ok("cron triggered"),
            Err(e) => Response::error(format!("cron failed: {e}"), 500),
        }
    } else {
        // Composition root: the HTTP runtime builds the D1-backed Store once and
        // injects it via Router::with_data so handlers read ctx.data instead of
        // constructing their own Store::new(env.d1(...)). Internal routes that
        // require adapters/state owned by worker-entry are registered here on the
        // api router (a consuming builder). Only HTTP composition/wiring lives in
        // worker-entry — the handlers delegate to application services.
        let store = match env.d1("DB") {
            Ok(db) => Store::new(db),
            Err(e) => {
                console_log!("[http] D1 binding failed: {e}");
                return Response::error("D1 unavailable", 503);
            }
        };
        let result = router(store)
            .post_async("/api/internal/context", crate::routes::context::internal_context)
            .post_async("/api/internal/agent/run", crate::routes::agent::run)
            // Signal read-model routes migrated out of api (P3 Round 2) — they
            // assemble D1SignalQuery in the composition root.
            .get_async("/api/intelligence/threads/:id", crate::routes::signal::thread_detail)
            .get_async("/api/intelligence/entities/:id/signals", crate::routes::signal::entities_signals)
            .get_async("/api/intelligence/entities/:id/threads", crate::routes::signal::entities_threads)
            // Infrastructure-facing endpoints migrated out of api (Phase 2) —
            // semantic/rebuild/search drive Vectorize + D1 FTS directly, and
            // reflection assembles its engine from worker env adapters.
            .post_async("/api/articles/search", crate::routes::semantic::semantic_search)
            .get_async("/api/articles/search", crate::routes::search::search_articles)
            .post_async("/api/admin/rebuild-embeddings", crate::routes::rebuild::rebuild_embeddings)
            .post_async("/api/intelligence/decisions/:id/reflect", crate::routes::reflection::reflect)
            .run(req, env)
            .await;
        if let Err(ref e) = result {
            console_log!("[ERROR] router.run failed: {e}");
        }
        result
    }
}
