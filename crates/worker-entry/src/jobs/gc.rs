use worker::*;
use store::Store;

/// Garbage-collect orphaned R2 objects.
///
/// Queries D1 for articles that match the expiry criteria and have a
/// raw_content_r2_key set, deletes the R2 objects first, then expires
/// the D1 rows.  This avoids accumulating orphaned R2 objects when
/// `expire_old_articles` removes the D1 row but not the corresponding
/// R2 blob.
///
/// Safe to call on every cron cycle — when there is nothing to expire
/// the D1 query returns zero rows and no R2 deletes happen.
pub(crate) async fn gc_r2_objects(env: &Env, now: i64) -> Result<u64, Error> {
    let bucket = match env.bucket("RAW_CONTENT") {
        Ok(b) => b,
        Err(_) => return Ok(0), // no R2 configured — skip
    };
    let store = Store::new(env.d1("DB").map_err(|e| Error::RustError(e.to_string()))?);

    // 1. Collect R2 keys for articles about to expire
    let r2_keys = store
        .expired_article_r2_keys(now, 30)
        .await
        .map_err(|e| Error::RustError(e.to_string()))?;

    if r2_keys.is_empty() {
        return Ok(0);
    }

    // 2. Delete the R2 objects first (before D1 rows are removed)
    let mut deleted_r2 = 0u64;
    for key in &r2_keys {
        if let Err(e) = bucket.delete(key).await {
            console_log!("[Sulix:gc] R2 delete failed for {key}: {e:?}");
        } else {
            deleted_r2 += 1;
        }
    }

    // 3. Expire the D1 rows
    let deleted_d1 = store
        .expire_old_articles(now, 30)
        .await
        .map_err(|e| Error::RustError(e.to_string()))?;

    console_log!("[Sulix:gc] deleted {deleted_r2} R2 objects, {deleted_d1} D1 rows");
    Ok(deleted_r2)
}
