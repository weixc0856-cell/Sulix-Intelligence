//! Stage 1: Preprocess — load all persistent state

use std::path::{Path, PathBuf};

use crate::archive::ChronicleDb;
use crate::config::Config;
use crate::engine::decision_registry::DecisionRegistry;
use crate::engine::memory::MemoryEngine;
use crate::storage;

/// Stage 1: Preprocess 装载的所有持久化状态
pub struct StateBundle {
    pub event_log: crate::event_log::EventLog,
    pub event_log_path: PathBuf,
    pub chronicle: Option<ChronicleDb>,
    pub chronicle_path: PathBuf,
    pub memory_for_linking: MemoryEngine,
    pub memory_path: PathBuf,
    pub registry: crate::engine::registry::AssessmentRegistry,
    pub registry_path: PathBuf,
    pub inv_registry: crate::engine::investigation_registry::InvestigationRegistry,
    pub inv_registry_path: PathBuf,
    pub decision_registry: DecisionRegistry,
}

/// Preprocess: 加载所有持久化状态（EventLog, Chronicle, Memory）
pub async fn publish_preprocess(data_dir: &Path, config: &Config) -> StateBundle {
    let event_log_path = data_dir.join("event_log.json");
    let event_log = load_or_new_event_log(&event_log_path);

    let chronicle_path = data_dir.join("database.json");
    let chronicle = if chronicle_path.exists() {
        match ChronicleDb::load(&chronicle_path) {
            Ok(c) => Some(c),
            Err(e) => {
                log::warn!("⚠️ Chronicle 加载失败: {}", e);
                None
            }
        }
    } else {
        None
    };

    let memory_path = PathBuf::from(&config.output.vault_path).join("memory_db.json");
    let mut memory_for_linking = MemoryEngine::new(memory_path.clone());
    if let Err(e) = memory_for_linking.load() {
        log::warn!("⚠️ Memory Engine 加载失败（用于冲突链接）: {}", e);
    }

    let registry_path = PathBuf::from(&config.output.vault_path).join("assessment_registry.json");
    let registry = crate::engine::registry::AssessmentRegistry::load_or_new(&registry_path);

    let inv_registry_path =
        PathBuf::from(&config.output.vault_path).join("investigation_registry.json");
    let inv_registry = crate::engine::investigation_registry::InvestigationRegistry::load_or_new(
        &inv_registry_path,
    );

    let decision_registry_path =
        PathBuf::from(&config.output.vault_path).join("decision_registry.json");
    let decision_registry = DecisionRegistry::load_or_new(&decision_registry_path);

    StateBundle {
        event_log,
        event_log_path,
        chronicle,
        chronicle_path,
        memory_for_linking,
        memory_path,
        registry,
        registry_path,
        inv_registry,
        inv_registry_path,
        decision_registry,
    }
}

fn load_or_new_event_log(path: &Path) -> crate::event_log::EventLog {
    storage::with_corrupt_recovery(
        path,
        |p| crate::event_log::EventLog::load_from_file(&p.to_string_lossy()),
        crate::event_log::EventLog::new,
    )
}
