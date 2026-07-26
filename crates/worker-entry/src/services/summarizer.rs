use crate::services::http_client::WorkerHttpClient;
use ai_pipeline::HttpSummarizer;
use model_runtime::RealDeepSeek;
use worker::*;

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
    let embedding_model = env.var("AI_EMBEDDING_MODEL").ok().map(|v| v.to_string()).unwrap_or_default();

    // Build the ModelProvider (RealDeepSeek) with Worker HTTP client
    let http_client = Box::new(WorkerHttpClient);
    let provider = Box::new(RealDeepSeek::new(base_url.clone(), api_key.clone(), chat_model, http_client));

    Some(HttpSummarizer::new(provider, embedding_model, base_url, api_key, Box::new(WorkerHttpClient)))
}
