//! Object Store — abstraction over cloud blob storage (R2).
//!
//! This crate provides a trait for reading and writing immutable objects,
//! plus production (R2) and test (in-memory) implementations.
//!
//! ## Bounded context
//!
//! This is **not** part of [`store`] — the `StoreBackend` trait abstracts
//! D1 (operational state), while `ObjectStore` abstracts R2 (memory archive).
//! They live in separate crates because they have different semantics:
//! query/join/aggregate vs. put/get/delete.

mod memory;
mod r2;

use async_trait::async_trait;

pub use memory::BlobStore;
pub use r2::R2Store;

/// Reference metadata returned after a successful object write.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectRef {
    /// The key at which the object was stored.
    pub key: String,
    /// Size of the object in bytes.
    pub size: usize,
    /// Unix timestamp of when the object was created.
    pub created_at: i64,
}

/// Errors from ObjectStore operations.
#[derive(Debug, thiserror::Error)]
pub enum ObjectStoreError {
    #[error("r2 error: {0}")]
    R2(String),
    #[error("object not found: {0}")]
    NotFound(String),
    #[error("empty body for key: {0}")]
    EmptyBody(String),
}

impl From<worker::Error> for ObjectStoreError {
    fn from(e: worker::Error) -> Self {
        ObjectStoreError::R2(e.to_string())
    }
}

/// Abstraction over cloud object storage.
///
/// Objects are **immutable** once written — the Memory Archive never
/// modifies an existing object in place.  Producers write once; readers
/// consume by key.
#[async_trait(?Send)]
pub trait ObjectStore {
    /// Write an immutable object. Returns an [`ObjectRef`] with metadata.
    async fn write_object(&self, key: &str, object: &[u8]) -> Result<ObjectRef, ObjectStoreError>;

    /// Read an object by key. Returns `None` when the key does not exist.
    async fn read_object(&self, key: &str) -> Result<Option<Vec<u8>>, ObjectStoreError>;

    /// Delete an object by key.
    async fn delete_object(&self, key: &str) -> Result<(), ObjectStoreError>;
}

/// Key prefix conventions for the Sulix Intelligence Memory Archive.
///
/// All objects live under `memory/` to namespace them separately from
/// ingestion artifacts (e.g. `articles/{id}`).
pub mod keys {
    /// Key for a decision artifact (reasoning, evaluation snapshot, etc).
    pub fn decision(decision_id: i64, filename: &str) -> String {
        format!("memory/decisions/{decision_id}/{filename}")
    }

    /// Key for an outcome event attached to a decision.
    pub fn decision_outcome(decision_id: i64, outcome_id: i64) -> String {
        format!("memory/decisions/{decision_id}/outcomes/{outcome_id}.json")
    }

    /// Key for a signal-level artifact (strategy output, etc).
    pub fn signal(signal_id: i64, filename: &str) -> String {
        format!("memory/signals/{signal_id}/{filename}")
    }

    /// Key for a single signal event in the event-sourced timeline.
    pub fn signal_event(thread_id: i64, timestamp: i64) -> String {
        format!("memory/signals/{thread_id}/events/{timestamp}.json")
    }

    /// Prefix for listing all events belonging to a signal thread.
    pub fn signal_event_prefix(thread_id: i64) -> String {
        format!("memory/signals/{thread_id}/events/")
    }

    /// Key for a daily briefing.
    pub fn briefing(date: &str) -> String {
        format!("memory/briefings/{date}.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        futures::executor::block_on(f)
    }

    #[test]
    fn blob_store_roundtrip() {
        let store = BlobStore::new();
        let data = b"hello, memory archive";
        let key = "memory/test/hello.json";

        let r = block_on(store.write_object(key, data)).unwrap();
        assert_eq!(r.key, key);
        assert_eq!(r.size, data.len());

        let read_back = block_on(store.read_object(key)).unwrap().expect("should exist");
        assert_eq!(read_back.as_slice(), data);

        let missing = block_on(store.read_object("nonexistent")).unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn blob_store_delete() {
        let store = BlobStore::new();
        block_on(store.write_object("memory/test/x", b"data")).unwrap();
        assert!(block_on(store.read_object("memory/test/x")).unwrap().is_some());

        block_on(store.delete_object("memory/test/x")).unwrap();
        assert!(block_on(store.read_object("memory/test/x")).unwrap().is_none());
    }

    #[test]
    fn keys_signal_event_roundtrip() {
        let k = keys::signal_event(42, 1710000000);
        assert_eq!(k, "memory/signals/42/events/1710000000.json");
        let prefix = keys::signal_event_prefix(42);
        assert_eq!(prefix, "memory/signals/42/events/");
    }

    #[test]
    fn keys_briefing() {
        let k = keys::briefing("2026-07-25");
        assert_eq!(k, "memory/briefings/2026-07-25.json");
    }

    #[test]
    fn keys_decision() {
        let k = keys::decision(1, "reasoning.json");
        assert_eq!(k, "memory/decisions/1/reasoning.json");
    }
}
