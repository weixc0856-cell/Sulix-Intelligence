//! Decision Registry — DEC-XXXX 稳定 ID 注册表
//!
//! 每个活跃 Assessment (ASM) 对应一个 canonical Decision (DEC)。
//! Registry 以 `asm_id` 为主键，确保 Decision 与 ASM 绑定而非与 thesis_id 绑定。
//!
//! 文件存储：vault_path/decision_registry.json

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::registry::RegistryCore;

/// Registry entry for a canonical Decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEntry {
    /// Primary canonical link: ASM-XXXX
    pub asm_id: String,
    /// Internal reference: thesis-XXXX
    pub thesis_id: String,
    pub created: String,
    pub updated: String,
    /// Lifecycle state: "active", "archived", "superseded", "expired"
    pub state: String,
    /// Latest decision type label for quick lookup
    pub current_type: String,
}

/// Decision Registry — maps DEC-XXXX → DecisionEntry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRegistry {
    #[serde(flatten)]
    pub core: RegistryCore,
    /// DEC-ID → DecisionEntry
    pub decisions: HashMap<String, DecisionEntry>,
}

impl Default for DecisionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionRegistry {
    pub fn new() -> Self {
        Self {
            core: RegistryCore::new(),
            decisions: HashMap::new(),
        }
    }

    /// Load from file, or return empty registry if not found (delegates to generic helper)
    pub fn load_or_new(path: &Path) -> Self {
        crate::engine::registry::load_or_new(path)
    }

    /// Persist to file (delegates to generic helper)
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        crate::engine::registry::save_json(self, path)
    }

    /// Find DEC-ID by ASM-ID (primary lookup)
    ///
    /// Returns the first active DEC. Use `find_all_by_asm` when ambiguity
    /// detection is required (1:many ASM→DEC).
    pub fn find_by_asm(&self, asm_id: &str) -> Option<String> {
        self.decisions
            .iter()
            .find(|(_, entry)| entry.asm_id == asm_id && entry.state == "active")
            .map(|(dec_id, _)| dec_id.clone())
    }

    /// Find all active DEC-IDs for a given ASM-ID.
    ///
    /// Used by detect_outcomes() to detect ambiguity (1:many ASM→DEC).
    /// Returns empty vec when no matches.
    pub fn find_all_by_asm(&self, asm_id: &str) -> Vec<String> {
        let mut ids: Vec<String> = self.decisions
            .iter()
            .filter(|(_, entry)| entry.asm_id == asm_id && entry.state == "active")
            .map(|(dec_id, _)| dec_id.clone())
            .collect();
        // Sort by created date (descending) via entry.updated, so latest comes first
        ids.sort_by(|a, b| {
            let a_date = self.decisions.get(a).map(|e| e.updated.as_str()).unwrap_or("");
            let b_date = self.decisions.get(b).map(|e| e.updated.as_str()).unwrap_or("");
            b_date.cmp(a_date) // descending
        });
        ids
    }

    /// Register a new Decision for an ASM, return DEC-ID
    pub fn register(
        &mut self,
        asm_id: &str,
        thesis_id: &str,
        today: &str,
        decision_type: &str,
    ) -> String {
        let dec_id = format!("DEC-{:04}", self.core.next_id);
        self.core.next_id += 1;
        self.decisions.insert(
            dec_id.clone(),
            DecisionEntry {
                asm_id: asm_id.to_string(),
                thesis_id: thesis_id.to_string(),
                created: today.to_string(),
                updated: today.to_string(),
                state: "active".to_string(),
                current_type: decision_type.to_string(),
            },
        );
        dec_id
    }

    /// Update the current_type and updated timestamp for an existing Decision
    pub fn update_type(&mut self, dec_id: &str, decision_type: &str, today: &str) {
        if let Some(entry) = self.decisions.get_mut(dec_id) {
            entry.current_type = decision_type.to_string();
            entry.updated = today.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_find_by_asm() {
        let mut reg = DecisionRegistry::new();
        let dec_id = reg.register("ASM-0001", "thesis-123", "2026-06-26", "monitor");
        assert_eq!(dec_id, "DEC-0001");
        let found = reg.find_by_asm("ASM-0001");
        assert_eq!(found, Some("DEC-0001".to_string()));
    }

    #[test]
    fn test_find_by_asm_no_match() {
        let reg = DecisionRegistry::new();
        assert!(reg.find_by_asm("ASM-9999").is_none());
    }

    #[test]
    fn test_update_type() {
        let mut reg = DecisionRegistry::new();
        let dec_id = reg.register("ASM-0001", "thesis-123", "2026-06-26", "monitor");
        reg.update_type(&dec_id, "build", "2026-06-27");
        let entry = reg.decisions.get(&dec_id).unwrap();
        assert_eq!(entry.current_type, "build");
        assert_eq!(entry.updated, "2026-06-27");
    }

    #[test]
    fn test_find_all_by_asm_single() {
        let mut reg = DecisionRegistry::new();
        reg.register("ASM-0001", "t1", "2026-07-01", "monitor");
        let all = reg.find_all_by_asm("ASM-0001");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0], "DEC-0001");
    }

    #[test]
    fn test_find_all_by_asm_none() {
        let reg = DecisionRegistry::new();
        assert!(reg.find_all_by_asm("ASM-9999").is_empty());
    }

    #[test]
    fn test_find_all_by_asm_returns_active_only() {
        let mut reg = DecisionRegistry::new();
        reg.register("ASM-0001", "t1", "2026-07-01", "monitor");
        // Manually set to non-active to simulate archived
        if let Some(e) = reg.decisions.get_mut("DEC-0001") {
            e.state = "archived".to_string();
        }
        assert!(reg.find_all_by_asm("ASM-0001").is_empty());
    }

    #[test]
    fn test_find_all_by_asm_multiple_warns() {
        let mut reg = DecisionRegistry::new();
        // Can't have two active for same ASM with current API, but test the method edge
        let d1 = reg.register("ASM-0001", "t1", "2026-07-01", "monitor");
        let d2 = reg.register("ASM-0001", "t2", "2026-07-02", "build");
        // Manually set first one to "active" as well (registry doesn't enforce unique)
        if let Some(e) = reg.decisions.get_mut(&d1) {
            e.state = "active".to_string();
        }
        // Now both are active — verify find_all returns both
        let all = reg.find_all_by_asm("ASM-0001");
        assert_eq!(all.len(), 2);
        // Latest first (sorted by updated descending)
        assert_eq!(all[0], d2);
        assert_eq!(all[1], d1);
    }

    #[test]
    fn test_register_increments_id() {
        let mut reg = DecisionRegistry::new();
        let id1 = reg.register("ASM-0001", "t1", "2026-06-26", "monitor");
        let id2 = reg.register("ASM-0002", "t2", "2026-06-26", "build");
        assert_eq!(id1, "DEC-0001");
        assert_eq!(id2, "DEC-0002");
        assert_eq!(reg.core.next_id, 3);
    }
}
