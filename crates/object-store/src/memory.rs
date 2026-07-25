//! In-memory ObjectStore for unit tests (replaces R2 in test contexts).

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;

use crate::{ObjectRef, ObjectStore, ObjectStoreError};

/// Unix timestamp (seconds) at the moment of the call.
fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Test-double ObjectStore backed by a `HashMap<String, Vec<u8>>`.
pub struct BlobStore {
    data: RefCell<HashMap<String, Vec<u8>>>,
}

impl BlobStore {
    pub fn new() -> Self {
        Self { data: RefCell::new(HashMap::new()) }
    }
}

impl Default for BlobStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl ObjectStore for BlobStore {
    async fn write_object(&self, key: &str, object: &[u8]) -> Result<ObjectRef, ObjectStoreError> {
        let now = now_secs();
        let size = object.len();
        self.data.borrow_mut().insert(key.to_string(), object.to_vec());
        Ok(ObjectRef { key: key.to_string(), size, created_at: now })
    }

    async fn read_object(&self, key: &str) -> Result<Option<Vec<u8>>, ObjectStoreError> {
        Ok(self.data.borrow().get(key).cloned())
    }

    async fn delete_object(&self, key: &str) -> Result<(), ObjectStoreError> {
        self.data.borrow_mut().remove(key);
        Ok(())
    }
}
