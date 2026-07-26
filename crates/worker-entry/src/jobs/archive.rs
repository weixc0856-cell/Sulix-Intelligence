//! Outbox Archive Worker — drains the `object_outbox` table and writes
//! pending entries to the R2 Memory Archive and EventStore.
//!
//! Types of outbox entries:
//!   event:*     — domain events → event_archive_index + R2
//!   archive:*   — artifacts → R2 only
//!   task:*      — job dispatch (consumed by cron, not this worker)
//!
//! Runs after the Signal Engine in the cron cycle.

use event_store::EventEnvelope;
use object_store::{ObjectStore, R2Store};
use store::D1Store;
use worker::*;

/// Drain up to 100 pending outbox entries.
pub(crate) async fn archive_outbox(env: &worker::Env) {
    let store = match env.d1("DB") {
        Ok(db) => D1Store::new(db),
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
        let success = if entry.object_type.starts_with("event:") {
            // Domain event: index + archive to R2
            handle_event_outbox(&store, &r2, entry).await
        } else {
            // Artifact archive: write to R2
            handle_archive_outbox(&r2, entry).await
        };

        if success {
            if let Err(e) = store.mark_outbox_archived(entry.id).await {
                console_log!("[archive] mark_archived failed for outbox {} (key={}): {e}", entry.id, entry.object_key);
            }
        } else {
            console_log!(
                "[archive] processing failed for outbox {} (key={}, type={})",
                entry.id,
                entry.object_key,
                entry.object_type
            );
            if let Err(db_err) = store.mark_outbox_failed(entry.id).await {
                console_log!("[archive] mark_failed failed for outbox {}: {db_err}", entry.id);
            }
        }
    }

    console_log!("[archive] processed {} outbox entries", entries.len());
}

/// Handle an event outbox entry: write to event_archive_index + R2 archive.
async fn handle_event_outbox(store: &D1Store, r2: &R2Store, entry: &store::OutboxEntry) -> bool {
    // Deserialize the EventEnvelope from the payload
    let event: EventEnvelope = match serde_json::from_str(&entry.payload) {
        Ok(e) => e,
        Err(e) => {
            console_log!("[archive] failed to deserialize event outbox {}: {e}", entry.id);
            return false;
        }
    };

    // 1. Write to event_archive_index (metadata for fast query)
    if let Err(e) = store
        .insert_event_index(
            &event.event_id,
            &event.aggregate.aggregate_type,
            &event.aggregate.aggregate_id,
            &event.event_type,
            &entry.object_key,
            event.occurred_at,
        )
        .await
    {
        console_log!("[archive] insert_event_index failed for outbox {}: {e}", entry.id);
        // Non-fatal: archive worker will retry
    }

    // 2. Write event payload to R2
    match r2.write_object(&entry.object_key, entry.payload.as_bytes()).await {
        Ok(_) => true,
        Err(e) => {
            console_log!("[archive] R2 write failed for event outbox {} (key={}): {e}", entry.id, entry.object_key);
            false
        }
    }
}

/// Handle an artifact outbox entry: write to R2 only.
async fn handle_archive_outbox(r2: &R2Store, entry: &store::OutboxEntry) -> bool {
    match r2.write_object(&entry.object_key, entry.payload.as_bytes()).await {
        Ok(_) => true,
        Err(e) => {
            console_log!("[archive] R2 write failed for archive outbox {} (key={}): {e}", entry.id, entry.object_key);
            false
        }
    }
}
