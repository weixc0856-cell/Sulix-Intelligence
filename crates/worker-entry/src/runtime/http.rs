use worker::*;

use crate::jobs::ingestion;
use api::router;
use application::ProductionAppServices;
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
        // Composition root: the HTTP runtime builds the D1-backed Store once,
        // wraps it in the application service bundle (ProductionAppServices),
        // and injects that via Router::with_data. api handlers read the exact
        // service they need from ctx.data; infra routes registered here reach
        // the raw store through ctx.data.store. Internal routes that require
        // adapters/state owned by worker-entry are registered on the api router
        // (a consuming builder). Only HTTP composition/wiring lives here — the
        // handlers delegate to application services.
        let store = match env.d1("DB") {
            Ok(db) => Store::new(db),
            Err(e) => {
                console_log!("[http] D1 binding failed: {e}");
                return Response::error("D1 unavailable", 503);
            }
        };
        let app = ProductionAppServices::new(store);
        let result = router(app)
            .post_async("/api/internal/context", crate::routes::context::internal_context)
            .post_async("/api/internal/agent/run", crate::routes::agent::run)
            // Signal read-model + radar routes migrated out of api — they
            // assemble D1SignalQuery / drive raw-store reads in the composition
            // root (raw Store reachable via ctx.data.store).
            .get_async("/api/intelligence/threads/:id", crate::routes::signal::thread_detail)
            .get_async("/api/intelligence/entities/:id/signals", crate::routes::signal::entities_signals)
            .get_async("/api/intelligence/entities/:id/threads", crate::routes::signal::entities_threads)
            .get_async("/api/intelligence/radar", crate::routes::signal::radar)
            .get_async("/api/intelligence/signals/:id", crate::routes::signal::signal_detail)
            .get_async("/api/intelligence/signals/:id/provenance", crate::routes::signal::signal_provenance)
            // Article raw content (R2 + content-governance policy) — infra route.
            .get_async("/api/articles/:id/content", crate::routes::article::article_content)
            // Infrastructure-facing endpoints migrated out of api (Phase 2) —
            // semantic/rebuild/search drive Vectorize + D1 FTS directly, and
            // reflection assembles its engine from worker env adapters.
            .post_async("/api/articles/search", crate::routes::semantic::semantic_search)
            .get_async("/api/articles/search", crate::routes::search::search_articles)
            .post_async("/api/admin/rebuild-embeddings", crate::routes::rebuild::rebuild_embeddings)
            .post_async("/api/intelligence/decisions/:id/reflect", crate::routes::reflection::reflect)
            // Decision-write vertical (Phase 2, Checkpoint F) — DecisionService
            // emits outbox events to EventStore, so writes belong here in the
            // composition root; api keeps the read handlers (DecisionReadService).
            .post_async("/api/intelligence/signals/:id/decisions", crate::routes::decision_write::create)
            .post_async("/api/intelligence/decisions/:id/status", crate::routes::decision_write::update_status)
            .post_async("/api/intelligence/decisions/:id/outcomes", crate::routes::decision_write::create_outcome)
            .post_async("/api/intelligence/decisions/:id/evaluations", crate::routes::decision_write::create_evaluation)
            .post_async("/api/decision-records", crate::routes::decision_write::create_decision_record)
            .post_async("/api/decision-records/:id/outcomes", crate::routes::decision_write::create_outcome_metric)
            // Briefing read endpoints (Phase 2, Checkpoint F) — orchestrate KV +
            // R2 Memory Archive from the worker env; D1 read is BriefingService.
            .get_async("/api/intelligence/briefing/today", crate::routes::briefing::today_briefing)
            .get_async("/api/intelligence/briefings", crate::routes::briefing::list_briefings)
            .get_async("/api/intelligence/briefings/:id", crate::routes::briefing::get_briefing)
            .run(req, env)
            .await;
        if let Err(ref e) = result {
            console_log!("[ERROR] router.run failed: {e}");
        }
        result
    }
}
