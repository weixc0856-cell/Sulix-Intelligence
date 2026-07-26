//! HTTP routes with CORS support for the Sulix Intelligence backend.
//! All responses include `Access-Control-Allow-Origin: *` so the API can
//! be consumed from the Astro frontend (even on a different domain) and
//! from browser-based dev tools without a proxy.

use worker::*;

// Backward-compatible re-exports for existing module files
pub(crate) use shared::cache::cache_get;
pub(crate) use shared::cache::cache_put;
pub(crate) use shared::params::fmt_date_ymd;
pub(crate) use shared::params::param_i64;
pub(crate) use shared::params::parse_limit;
pub(crate) use shared::params::parse_offset;
pub(crate) use shared::response::cors_headers;
pub(crate) use shared::response::json_err;
pub(crate) use shared::response::json_err_internal;
pub(crate) use shared::response::json_ok;
pub use store::Store;

mod briefing;
mod entities;
mod rebuild;
mod routes;
mod semantic;
mod services;
mod shared;
mod strategies;

pub fn router() -> Router<'static, ()> {
    Router::new()
        // CORS preflight
        .options_async("/api/*path", routes::system::cors_preflight)
        // Health / debug
        .get_async("/api/ping", routes::system::ping)
        .get_async("/api/pipeline/status", routes::system::pipeline_status)
        .get_async("/api/health", routes::system::health)
        .get_async("/api/debug/feeds-due", routes::system::debug_feeds_due)
        // Signal Strategies preview
        .post_async("/api/strategies/preview", strategies::preview)
        .post_async("/api/articles/search", semantic::semantic_search)
        .post_async("/api/admin/rebuild-embeddings", rebuild::rebuild_embeddings)
        // Aggregations
        .get_async("/api/dashboard", routes::system::dashboard)
        .get_async("/api/stats", routes::system::stats)
        .get_async("/api/categories", routes::system::categories)
        .get_async("/api/tags", routes::system::tags)
        .get_async("/api/intelligence/signals", routes::system::intelligence_signals)
        .get_async("/api/intelligence/radar", routes::signal::radar)
        .get_async("/api/intelligence/signals/:id", routes::signal::signal_detail)
        .get_async("/api/intelligence/threads/:id", routes::signal::thread_detail)
        .get_async("/api/intelligence/briefing/today", briefing::today_briefing)
        .get_async("/api/intelligence/briefings", briefing::list_briefings)
        .get_async("/api/intelligence/briefings/:id", briefing::get_briefing)
        // Entity Graph
        .get_async("/api/intelligence/entities", entities::entities_list)
        .get_async("/api/intelligence/entities/:id", entities::entities_get)
        .get_async("/api/intelligence/entities/:id/activity", entities::entities_activity)
        .get_async("/api/intelligence/entities/:id/articles", entities::entities_articles)
        .get_async("/api/intelligence/entities/:id/signals", entities::entities_signals)
        .get_async("/api/intelligence/entities/:id/relations", entities::entities_get_relations)
        .get_async("/api/intelligence/entities/:id/threads", entities::entities_threads)
        // Decision Loop
        .get_async("/api/intelligence/decisions", routes::decision::list)
        .get_async("/api/intelligence/decisions/stats", routes::decision::stats)
        .get_async("/api/intelligence/decisions/:id", routes::decision::detail)
        .post_async("/api/intelligence/decisions/:id/status", routes::decision::update_status)
        .post_async("/api/intelligence/decisions/:id/reflect", routes::reflection::reflect)
        .post_async("/api/intelligence/decisions/:id/outcomes", routes::decision::create_outcome)
        .get_async("/api/intelligence/decisions/:id/outcomes", routes::decision::list_outcomes)
        .post_async("/api/intelligence/decisions/:id/evaluations", routes::decision::create_evaluation)
        .get_async("/api/intelligence/decisions/:id/evaluations", routes::decision::list_evaluations)
        .get_async("/api/intelligence/decisions/:id/timeline", routes::decision::timeline)
        .post_async("/api/intelligence/signals/:id/decisions", routes::decision::create)
        .get_async("/api/intelligence/signals/:id/decisions", routes::decision::by_signal)
        // Feed CRUD
        .get_async("/api/feeds", routes::feed::feeds_list)
        .post_async("/api/feeds", routes::feed::feeds_create)
        .get_async("/api/feeds/:id", routes::feed::feeds_get)
        .put_async("/api/feeds/:id", routes::feed::feeds_update)
        .delete_async("/api/feeds/:id", routes::feed::feeds_delete)
        // Article endpoints
        .get_async("/api/articles/latest", routes::article::latest_articles)
        .get_async("/api/articles/trending", routes::article::trending)
        .get_async("/api/articles/batch", routes::article::articles_batch)
        .get_async("/api/articles/search", routes::article::search_articles)
        .get_async("/api/articles/:id/related", routes::article::article_related)
        .get_async("/api/articles/:id/adjacent", routes::article::article_adjacent)
        .get_async("/api/articles/:id", routes::article::article_detail)
        .get_async("/api/articles/:id/content", routes::article::article_content)
        // Context Engine
        .post_async("/api/internal/context", routes::context::internal_context)
        // Agent Reasoning Engine
        .post_async("/api/internal/agent/run", routes::agent::run)
        // Decision Graph Projection
        .get_async("/api/projections/decision-graph", routes::graph::decision_graph)
        .post_async("/api/projections/decision-graph/:id/expand", routes::graph::expand)
        .get_async("/api/claims/:id", routes::claim::detail_with_evidence)
        // Rules CRUD
        .get_async("/api/rules", routes::rules::rules_list)
        .post_async("/api/rules", routes::rules::rules_create)
        .get_async("/api/rules/:id", routes::rules::rules_get)
        .put_async("/api/rules/:id", routes::rules::rules_update)
        .delete_async("/api/rules/:id", routes::rules::rules_delete)
}
