use crate::services::http_client::WorkerHttpClient;
use ai_pipeline::HttpSummarizer;
use model_runtime::RealDeepSeek;
use worker::*;

/// Build the summarizer (DeepSeek chat via ModelProvider) from env vars.
///
/// Embedding configuration deliberately does NOT live here: summarization and
/// embedding are two separate seams. The embedder (Workers AI) is built in
/// `services::embedder` and run by the jobs after `process_article` persists
/// the summary.
pub fn try_build_summarizer(env: &Env) -> Option<HttpSummarizer> {
    let api_key = match env.secret("AI_API_KEY") {
        Ok(v) => v.to_string(),
        Err(_) => {
            console_log!("AI_API_KEY not set");
            return None;
        }
    };
    let base_url =
        env.var("AI_BASE_URL").ok().map(|v| v.to_string()).unwrap_or_else(|| "https://api.deepseek.com/v1".into());
    let chat_model = env.var("AI_CHAT_MODEL").ok().map(|v| v.to_string()).unwrap_or_else(|| "deepseek-v4-flash".into());

    // Build the ModelProvider (RealDeepSeek) with Worker HTTP client
    let http_client = Box::new(WorkerHttpClient);
    let provider = Box::new(RealDeepSeek::new(base_url, api_key, chat_model, http_client));

    Some(HttpSummarizer::new(provider))
}
