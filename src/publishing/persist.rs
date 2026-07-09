//! Stage 4: Persist — all JSON/SQLite persistence

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::db::Database;

use super::preprocess::StateBundle;
use super::infer::InferredState;

/// Persist: 所有持久化写入（Memory, Registry, EntityDb, Decision, EventLog, SQLite report）
pub async fn publish_persist(
    db: &Database,
    data_dir: &Path,
    today: &str,
    entity_db: &mut crate::entity::EntitySanctionDb,
    state: &mut StateBundle,
    inferred: &mut InferredState,
    config: &Config,
) {
    // Memory Engine save
    if let Err(e) = inferred.memory.save() {
        log::warn!("⚠️ Memory Engine 保存失败: {}", e);
    }

    // Assessment Registry save
    if let Err(e) = state.registry.save(&state.registry_path) {
        log::warn!("⚠️ Assessment Registry 保存失败: {}", e);
    } else {
        log::info!("📋 Assessment Registry: {} assessments, next ID: ASM-{:04}",
            state.registry.assessments.len(), state.registry.core.next_id);
    }

    // Decision Registry save
    let dec_registry_path = PathBuf::from(&config.output.vault_path).join("decision_registry.json");
    let mut dec_registry = crate::engine::decision_registry::DecisionRegistry::load_or_new(&dec_registry_path);
    {
        let theses_snapshot: Vec<_> = inferred.memory.theses().iter()
            .map(|t| (t.id.clone(), t.assessment_id.clone())).collect();
        for td in &inferred.thesis_decisions {
            if let Some((_, Some(asm_id))) = theses_snapshot.iter().find(|(id, _)| id == &td.thesis_id) {
                if let Some(event) = inferred.memory.record_or_update_decision(td, asm_id, today, &mut dec_registry) {
                    inferred.events.push(event);
                }
            }
        }
    }
    if let Err(e) = dec_registry.save(&dec_registry_path) {
        log::warn!("⚠️ Decision Registry 保存失败: {}", e);
    } else {
        log::info!("🎯 Decision Registry: {} decisions, next ID: DEC-{:04}",
            dec_registry.decisions.len(), dec_registry.core.next_id);
    }

    // Investigation Registry save
    if let Err(e) = state.inv_registry.save(&state.inv_registry_path) {
        log::warn!("⚠️ Investigation Registry 保存失败: {}", e);
    } else {
        log::info!("📋 Investigation Registry: {} investigations, next ID: INV-{:04}",
            state.inv_registry.investigations.len(), state.inv_registry.core.next_id);
    }

    // EntitySanctionDb save
    let entity_db_path = data_dir.join("entity_db.json");
    if let Err(e) = entity_db.save_to_file(&entity_db_path.to_string_lossy()) {
        log::warn!("⚠️ EntitySanctionDb 保存失败: {}", e);
    }

    // SQLite report
    if let Err(e) = db.record_report(today, &format!("Daily brief - {} topics", inferred.memory.theses().len()), 0) {
        log::warn!("⚠️ DB report 记录失败: {}", e);
    }
}
