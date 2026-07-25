//! R2-backed ObjectStore implementation.
//!
//! Wraps a `worker::Bucket` obtained from `env.bucket("RAW_CONTENT")`.

use crate::{ObjectRef, ObjectStore, ObjectStoreError};
use async_trait::async_trait;
use worker::Bucket;

/// Production object store backed by Cloudflare R2.
pub struct R2Store {
    bucket: Bucket,
}

impl R2Store {
    pub fn new(bucket: Bucket) -> Self {
        Self { bucket }
    }
}

#[async_trait(?Send)]
impl ObjectStore for R2Store {
    async fn write_object(&self, key: &str, object: &[u8]) -> Result<ObjectRef, ObjectStoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let size = object.len();

        self.bucket
            .put(key, object.to_vec())
            .execute()
            .await
            .map_err(|e| ObjectStoreError::R2(format!("r2 put failed for {key}: {e}")))?;

        Ok(ObjectRef { key: key.to_string(), size, created_at: now })
    }

    async fn read_object(&self, key: &str) -> Result<Option<Vec<u8>>, ObjectStoreError> {
        let object = self
            .bucket
            .get(key)
            .execute()
            .await
            .map_err(|e| ObjectStoreError::R2(format!("r2 get failed for {key}: {e}")))?;

        match object {
            Some(obj) => {
                let bytes = obj
                    .body()
                    .ok_or_else(|| ObjectStoreError::EmptyBody(key.to_string()))?
                    .bytes()
                    .await
                    .map_err(|e| ObjectStoreError::R2(format!("r2 body read failed for {key}: {e}")))?;
                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }

    async fn delete_object(&self, key: &str) -> Result<(), ObjectStoreError> {
        self.bucket
            .delete(key)
            .await
            .map_err(|e| ObjectStoreError::R2(format!("r2 delete failed for {key}: {e}")))
    }
}
