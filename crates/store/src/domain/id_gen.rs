//! ID Generator — deterministic sequential ID generation for MemoryStore.
//!
//! Replaces the ad-hoc `next_xxx_id` RefCell fields in MemoryStore with a
//! unified generator keyed by entity type name.

use std::cell::RefCell;
use std::collections::HashMap;

/// Generates sequential IDs for in-memory test stores.
pub trait IdGenerator {
    fn next(&self, entity: &str) -> i64;
}

/// Production implementation wrapping a `RefCell<HashMap>`.
///
/// Each entity type gets its own counter starting at 1.
#[derive(Default)]
pub struct SequentialIdGen {
    counters: RefCell<HashMap<String, i64>>,
}

impl SequentialIdGen {
    pub fn new() -> Self {
        Self { counters: RefCell::new(HashMap::new()) }
    }
}

impl IdGenerator for SequentialIdGen {
    fn next(&self, entity: &str) -> i64 {
        let mut map = self.counters.borrow_mut();
        let next = map.get(entity).copied().unwrap_or(1);
        map.insert(entity.to_string(), next + 1);
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_ids() {
        let gen = SequentialIdGen::new();
        assert_eq!(gen.next("article"), 1);
        assert_eq!(gen.next("article"), 2);
        assert_eq!(gen.next("entity"), 1);
        assert_eq!(gen.next("article"), 3);
    }
}
