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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_secs_round_trips() {
        let t = Timestamp::from_epoch_secs(1_752_000_000);
        assert_eq!(t.epoch_secs(), 1_752_000_000);
    }

    #[test]
    fn handles_zero_and_negative_epoch() {
        assert_eq!(Timestamp::from_epoch_secs(0).epoch_secs(), 0);
        assert_eq!(Timestamp::from_epoch_secs(-1).epoch_secs(), -1);
    }

    #[test]
    fn timestamps_are_ordered_and_copy() {
        let a = Timestamp::from_epoch_secs(100);
        let b = Timestamp::from_epoch_secs(200);
        assert!(a < b);
        let c = a; // Copy — `a` is still usable
        assert_eq!(c, a);
    }

    #[test]
    fn timestamp_serde_round_trips() {
        let t = Timestamp::from_epoch_secs(1_752_000_000);
        let json = serde_json::to_string(&t).unwrap();
        let back: Timestamp = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }
}
