use worker::*;

pub(crate) async fn cache_get(env: &Env, key: &str) -> Option<String> {
    let kv = env.kv("CACHE").ok()?;
    kv.get(key).text().await.ok().flatten()
}

pub(crate) async fn cache_put(env: &Env, key: &str, value: &str, ttl: u64) {
    if let Ok(kv) = env.kv("CACHE") {
        if let Ok(builder) = kv.put(key, value) {
            let _ = builder.expiration_ttl(ttl).execute().await;
        }
    }
}
