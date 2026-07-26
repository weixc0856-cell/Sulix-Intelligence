//! Shared time abstractions.
//!
//! Currently a thin wrapper around Unix-epoch seconds (`i64`).
//! Future: `chrono::DateTime<Utc>` once we remove the `js-sys` dependency.

use serde::{Deserialize, Serialize};

/// A point in time, stored as Unix-epoch seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

impl Timestamp {
    pub const fn from_epoch_secs(secs: i64) -> Self {
        Self(secs)
    }

    pub fn epoch_secs(&self) -> i64 {
        self.0
    }
}
