//! Decision Registry — DEC-XXXX 稳定 ID 注册表
//!
//! 每个活跃 Assessment (ASM) 对应一个 canonical Decision (DEC)。
//! Registry 以 `asm_id` 为主键，确保 Decision 与 ASM 绑定而非与 thesis_id 绑定。
//!
//! 文件存储：vault_path/decision_registry.json

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

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
    pub next_id: u32,
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
            next_id: 1,
            decisions: HashMap::new(),
        }
    }

    /// Load from file, or return empty registry if not found
    pub fn load_or_new(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Persist to file
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Find DEC-ID by ASM-ID (primary lookup)
    pub fn find_by_asm(&self, asm_id: &str) -> Option<String> {
        self.decisions
            .iter()
            .find(|(_, entry)| entry.asm_id == asm_id && entry.state == "active")
            .map(|(dec_id, _)| dec_id.clone())
    }

    /// Register a new Decision for an ASM, return DEC-ID
    pub fn register(
        &mut self,
        asm_id: &str,
        thesis_id: &str,
        today: &str,
        decision_type: &str,
    ) -> String {
        let dec_id = format!("DEC-{:04}", self.next_id);
        self.next_id += 1;
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
    fn test_register_increments_id() {
        let mut reg = DecisionRegistry::new();
        let id1 = reg.register("ASM-0001", "t1", "2026-06-26", "monitor");
        let id2 = reg.register("ASM-0002", "t2", "2026-06-26", "build");
        assert_eq!(id1, "DEC-0001");
        assert_eq!(id2, "DEC-0002");
        assert_eq!(reg.next_id, 3);
    }
}
