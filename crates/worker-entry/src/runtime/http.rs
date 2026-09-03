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
            .run(req, env)
            .await;
        if let Err(ref e) = result {
            console_log!("[ERROR] router.run failed: {e}");
        }
        result
    }
}
