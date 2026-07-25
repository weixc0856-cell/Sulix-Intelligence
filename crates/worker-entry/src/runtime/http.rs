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
        let result = router().run(req, env).await;
        if let Err(ref e) = result {
            console_log!("[ERROR] router.run failed: {e}");
        }
        result
    }
}
