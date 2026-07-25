//! Outbox Archive Worker — drains the `object_outbox` table and writes
//! pending archive entries to the R2 Memory Archive.
//!
//! Runs after the Signal Engine in the cron cycle so that any decision/
//! signal artifacts produced by the current cycle are included in the
//! drain batch.

use worker::*;

use object_store::{ObjectStore, R2Store};
use store::Store;

/// Drain up to 100 pending outbox entries, writing each to R2.
///
/// On success the entry is marked `archived` in the outbox table.
/// On R2 failure the entry is left `pending` and will be retried on
/// the next cycle (up to 3 retries, then moved to `failed`).
pub(crate) async fn archive_outbox(env: &worker::Env) {
    let store = match env.d1("DB") {
        Ok(db) => Store::new(db),
        Err(e) => {
            console_log!("[archive] D1 binding failed: {e}");
            return;
        }
    };
    let r2 = match env.bucket("RAW_CONTENT").ok() {
        Some(bucket) => R2Store::new(bucket),
        None => {
            console_log!("[archive] RAW_CONTENT bucket not bound — skipping");
            return;
        }
    };

    let entries = match store.drain_outbox(100).await {
        Ok(e) => e,
        Err(e) => {
            console_log!("[archive] drain_outbox failed: {e}");
            return;
        }
    };

    if entries.is_empty() {
        return;
    }

    for entry in &entries {
        let payload = entry.payload.as_bytes();
        match r2.write_object(&entry.object_key, payload).await {
            Ok(_) => {
                if let Err(e) = store.mark_outbox_archived(entry.id).await {
                    console_log!(
                        "[archive] mark_archived failed for outbox {} (key={}): {e}",
                        entry.id,
                        entry.object_key
                    );
                }
            }
            Err(e) => {
                console_log!(
                    "[archive] R2 write failed for outbox {} (key={}): {e}",
                    entry.id,
                    entry.object_key
                );
                if let Err(db_err) = store.mark_outbox_failed(entry.id).await {
                    console_log!(
                        "[archive] mark_failed failed for outbox {}: {db_err}",
                        entry.id
                    );
                }
            }
        }
    }

    console_log!("[archive] processed {} outbox entries", entries.len());
}
