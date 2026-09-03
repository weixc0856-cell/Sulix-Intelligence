use worker::*;

use crate::jobs::ingestion;
use api::router;

pub(crate) async fn handle(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    console_log!("[INFO] HTTP request: {} {}", req.method(), req.path());
    if req.path().to_lowercase().contains("__cron") {
        match ingestion::process_all_feeds(&env).await {
            Ok(_) => Response::ok("cron triggered"),
            Err(e) => Response::error(format!("cron failed: {e}"), 500),
        }
    } else {
        // Composition-root route injection: internal routes that require
        // adapters/state owned by worker-entry are registered here on the api
        // router (a consuming builder). Only HTTP composition/wiring lives in
        // worker-entry — the handlers delegate to application services.
        let result = router()
            .post_async("/api/internal/context", crate::routes::context::internal_context)
            .post_async("/api/internal/agent/run", crate::routes::agent::run)
            // Signal read-model routes migrated out of api (P3 Round 2) — they
            // assemble D1SignalQuery in the composition root.
            .get_async("/api/intelligence/threads/:id", crate::routes::signal::thread_detail)
            .get_async("/api/intelligence/entities/:id/signals", crate::routes::signal::entities_signals)
            .get_async("/api/intelligence/entities/:id/threads", crate::routes::signal::entities_threads)
            .run(req, env)
            .await;
        if let Err(ref e) = result {
            console_log!("[ERROR] router.run failed: {e}");
        }
        result
    }
}
