//! Host-error conversion helper.
//!
//! `domain::StoreError` is host-agnostic (no `From<worker::Error>`, since that
//! would couple the infra-free `domain` crate to the Cloudflare host). The D1
//! access layer converts `worker::Error` sites via this extension trait instead.

use worker::Error as WorkerError;

use crate::StoreError;

/// Convert a `Result<_, worker::Error>` into a `Result<_, StoreError>`.
pub(crate) trait StoreResultExt<T> {
    /// Map a `worker::Error` into [`StoreError::D1`].
    fn s_err(self) -> Result<T, StoreError>;
}

impl<T> StoreResultExt<T> for Result<T, WorkerError> {
    fn s_err(self) -> Result<T, StoreError> {
        self.map_err(|e| StoreError::D1(e.to_string()))
    }
}
