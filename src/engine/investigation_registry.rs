//! Investigation Registry — INV-XXXX 稳定 ID 注册表
//!
//! 每个 Assessment (ASM) 可以拥有多个 Investigation（0..N），
//! 随着证据积累而版本演化（旧 INV 被 Superseded）。
//!
//! 设计原则：
//!   - Registry 只存储 ID + 映射关系，不存储业务数据
//!   - 主键 = ASM-ID（canonical），thesis_id 仅作内部引用
//!   - 支持同一 ASM 对应多个历史 INV（supersede 关系）
//!
//! 文件存储：vault_path/investigation_registry.json

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::registry::RegistryCore;

/// Registry entry — identity and mapping only, no business data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigationEntry {
    /// Primary canonical link: ASM-XXXX
    pub asm_id: String,
    /// Internal reference: thesis-XXXX
    pub thesis_id: String,
    pub created: String,
    pub updated: String,
    /// Lifecycle state: "active", "completed", "superseded", "archived"
    pub state: String,
}

/// Investigation Registry — maps INV-XXXX → InvestigationEntry
///
/// Multiple INV-IDs can map to the same ASM-ID (version history).
/// Only one should be "active" at a time per ASM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigationRegistry {
    #[serde(flatten)]
    pub core: RegistryCore,
    /// INV-ID → InvestigationEntry
    pub investigations: HashMap<String, InvestigationEntry>,
}

impl Default for InvestigationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InvestigationRegistry {
    pub fn new() -> Self {
        Self {
            core: RegistryCore::new(),
            investigations: HashMap::new(),
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

    /// Find the currently active INV-ID for a given ASM
    pub fn find_active_by_asm(&self, asm_id: &str) -> Option<String> {
        self.investigations
            .iter()
            .find(|(_, entry)| entry.asm_id == asm_id && entry.state == "active")
            .map(|(inv_id, _)| inv_id.clone())
    }

    /// Register a new Investigation for an ASM, return INV-ID
    pub fn register(&mut self, asm_id: &str, thesis_id: &str, today: &str) -> String {
        let inv_id = format!("INV-{:04}", self.core.next_id);
        self.core.next_id += 1;
        self.investigations.insert(
            inv_id.clone(),
            InvestigationEntry {
                asm_id: asm_id.to_string(),
                thesis_id: thesis_id.to_string(),
                created: today.to_string(),
                updated: today.to_string(),
                state: "active".to_string(),
            },
        );
        inv_id
    }

    /// Supersede the old INV and register a new one for the same ASM.
    /// Old INV state → "superseded", new INV state → "active".
    pub fn supersede_and_register(
        &mut self,
        old_inv_id: &str,
        asm_id: &str,
        thesis_id: &str,
        today: &str,
    ) -> String {
        if let Some(entry) = self.investigations.get_mut(old_inv_id) {
            entry.state = "superseded".to_string();
            entry.updated = today.to_string();
        }
        self.register(asm_id, thesis_id, today)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_find_active() {
        let mut reg = InvestigationRegistry::new();
        let inv_id = reg.register("ASM-0001", "thesis-123", "2026-06-26");
        assert_eq!(inv_id, "INV-0001");
        assert_eq!(reg.find_active_by_asm("ASM-0001"), Some("INV-0001".to_string()));
    }

    #[test]
    fn test_find_active_no_match() {
        let reg = InvestigationRegistry::new();
        assert!(reg.find_active_by_asm("ASM-9999").is_none());
    }

    #[test]
    fn test_supersede_and_register() {
        let mut reg = InvestigationRegistry::new();
        let inv1 = reg.register("ASM-0001", "thesis-123", "2026-06-01");
        assert_eq!(inv1, "INV-0001");

        let inv2 = reg.supersede_and_register(&inv1, "ASM-0001", "thesis-123", "2026-06-26");
        assert_eq!(inv2, "INV-0002");

        // Old is superseded
        assert_eq!(reg.investigations["INV-0001"].state, "superseded");
        // New is active
        assert_eq!(reg.find_active_by_asm("ASM-0001"), Some("INV-0002".to_string()));
    }

    #[test]
    fn test_register_increments_id() {
        let mut reg = InvestigationRegistry::new();
        let id1 = reg.register("ASM-0001", "t1", "2026-06-26");
        let id2 = reg.register("ASM-0002", "t2", "2026-06-26");
        assert_eq!(id1, "INV-0001");
        assert_eq!(id2, "INV-0002");
        assert_eq!(reg.core.next_id, 3);
    }
}
