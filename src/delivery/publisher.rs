//! Delivery Publisher — 验证门 + 本地写入 + R2 上传 + 前端同步
//!
//! 内部顺序固定（不是 trait 分发）：
//!   1. Schema validation（逐对象）
//!   2. 拒绝对象写入 data/rejected/{date}/
//!   3. 补发 publish_rejected 事件
//!   4. Manifest 生成（此时计数 = 真实上传数）
//!   5. Local write（本地写入既有逻辑）
//!   6. R2 upload（如配置）
//!   7. 返回 PublishReport
//!
//! 错误分层：
//!   - 验证拒绝 → 按对象粒度 reject，计入 PublishReport
//!   - 本地写入失败 → 硬错误，直接 ?
//!   - R2 网络失败 → 记入 r2_status，不 ?，但触发非零退出

use anyhow::Result;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::artifact::manifest::ContentManifest;
use crate::config::Config;
use crate::domain::artifact::ArtifactSet;
use crate::event_log::{ObjectEvent, ObjectEventType};
use crate::schema::validator::{ValidationResult, Validate};
use crate::translation::TranslationCoverage;

/// 发布包 — 包含所有层级的产出
pub struct PublishBundle {
    pub research: ArtifactSet,
    pub intel_paths: Vec<PathBuf>,
    pub raw_count: usize,
    pub funnel_fetched: usize,
    pub funnel_deduped: usize,
    pub llm_calls: u64,
}

impl PublishBundle {
    /// 从各层产出组装发布包
    pub fn new(
        research: ArtifactSet,
        intel_paths: Vec<PathBuf>,
        raw_count: usize,
        funnel_fetched: usize,
        funnel_deduped: usize,
        llm_calls: u64,
    ) -> Self {
        Self {
            research,
            intel_paths,
            raw_count,
            funnel_fetched,
            funnel_deduped,
            llm_calls,
        }
    }
}

/// 发布报告
#[derive(Debug, Clone)]
pub struct PublishReport {
    pub passed_count: usize,
    pub rejected_count: usize,
    pub r2_status: String,
    pub manifest_version: u32,
    pub translation_coverage: Option<TranslationCoverage>,
}

impl PublishReport {
    /// R2 上传是否失败（检查状态字符串前缀）
    /// 腐化面仅限于此：新增 r2_status 输出值时必须更新此方法。
    pub fn r2_failed(&self) -> bool {
        self.r2_status.starts_with("failed") || self.r2_status.starts_with("partial_failure")
    }
}

/// 写 rejected 对象完整快照（含 errors + 对象本体，不进 R2）
fn write_rejected_snapshot<T: Serialize>(
    obj: &T,
    result: &ValidationResult,
    rejected_dir: &Path,
) {
    if let Ok(obj_json) = serde_json::to_value(obj) {
        let snapshot = serde_json::json!({
            "object": obj_json,
            "object_type": result.object_type,
            "object_id": result.object_id,
            "errors": result.errors,
            "warnings": result.warnings,
        });
        let path = rejected_dir.join(format!("{}-{}.json", result.object_type, result.object_id));
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&snapshot).unwrap_or_default());
        log::warn!("📋 Rejected: {} {} ({:?})", result.object_type, result.object_id, result.errors);
    }
}

/// 发布产物到存储后端
///
/// ownership 从 publishing 移交：PublishBundle 被 consume
pub async fn publish(
    bundle: PublishBundle,
    config: &Config,
    data_dir: &Path,
    today: &str,
) -> Result<PublishReport> {
    let artifacts = bundle.research;
    let mut passed = 0usize;
    let mut rejected = 0usize;

    // 1. Schema validation — 对象级验证（audit-mode: detects, records, alerts — does not yet block）
    // TODO(STEP-3.6): Move validation before emit. Rejected objects skip rendering.
    // TODO(STEP-4): Add cross-object reference validation (e.g. Decision→Assessment existence).
    let rejected_dir = data_dir.join("rejected").join(today);
    std::fs::create_dir_all(&rejected_dir)?;

    let mut report = crate::schema::validator::ValidationReport::new(today);

    // validation + rejected snapshot 在同一个循环内（防止索引偏移）
    for obj in &artifacts.assessment_objects {
        let result = obj.check();
        let passed_obj = result.passed;
        report.add_result(result);
        if passed_obj {
            passed += 1;
        } else {
            rejected += 1;
            write_rejected_snapshot(obj, &report.results.last().unwrap(), &rejected_dir);
        }
    }
    for obj in &artifacts.decision_objects {
        let result = obj.check();
        let passed_obj = result.passed;
        report.add_result(result);
        if passed_obj {
            passed += 1;
        } else {
            rejected += 1;
            write_rejected_snapshot(obj, &report.results.last().unwrap(), &rejected_dir);
        }
    }

    // 写入验证报告
    let validation_dir = data_dir.join("validation");
    std::fs::create_dir_all(&validation_dir)?;
    let report_path = validation_dir.join(format!("{}.json", today));
    if let Err(e) = report.save(&report_path) {
        log::warn!("⚠️ Validation report save failed: {}", e);
    }

    // 验证各种 artifact 计数
    let total_assessments = artifacts.assessment_count;
    let total_investigations = artifacts.investigation_count;
    let decision_count = artifacts.thesis_decisions.len();

    // 2. 补发 publish_rejected 事件（审计线不中断）

    // 3. 生成 manifest（此刻计数 = 验证通过后的真实上传数）
    let manifest = ContentManifest::new(
        today,
        0, // prev_version — will be read from existing file
        config.output.frontend_public_dir.clone(),
        "healthy",
        None, // observation_count
        None, // signal_count
        None, // duration_seconds
        None, // stages
    );

    // 回填真实计数
    let manifest = manifest
        .with_counts(
            total_assessments.saturating_sub(rejected),
            total_investigations,
            decision_count,
            artifacts.archive_days,
            artifacts.total_signals,
        )
        .with_funnel(
            bundle.funnel_fetched,
            bundle.funnel_deduped,
            bundle.funnel_fetched, // scored = fetched (same count in Phase 0)
            bundle.intel_paths.len(),
            total_assessments.saturating_sub(rejected),
            bundle.llm_calls,
        );

    // 4. Local write — manifest to vault_path and frontend public/ (NOT to mdx_dir — that is Astro content root)
    {
        let vault_manifest_path = PathBuf::from(&config.output.vault_path).join("manifest.json");
        if let Err(e) = manifest.save_as_json(&vault_manifest_path) {
            log::warn!("⚠️ Manifest vault save failed: {}", e);
        } else {
            log::info!("📋 Manifest v{} saved", manifest.version);
        }

        // Sync to frontend public/ if configured
        if let Some(ref fe_public_dir) = config.output.frontend_public_dir {
            let fe_path = PathBuf::from(fe_public_dir).join("manifest.json");
            if let Err(e) = manifest.save_as_json(&fe_path) {
                log::warn!("⚠️ Manifest frontend sync failed: {}", e);
            } else {
                log::info!("📋 Manifest synced to frontend public/");
            }
        }

        // Also sync to mdx_dir so R2 upload (Phase 1) can find it
        if let Some(ref mdx_dir) = config.output.mdx_dir {
            let mdx_manifest = PathBuf::from(mdx_dir).join("manifest.json");
            if let Err(e) = manifest.save_as_json(&mdx_manifest) {
                log::warn!("⚠️ Manifest mdx_dir sync failed: {}", e);
            } else {
                log::debug!("📋 Manifest synced to mdx_dir");
            }
        }
    }

    // 5. R2 upload (if configured — soft error, recorded in r2_status)
    // r2_client 保留到函数尾部，用于 flush 后补传 events/ 文件
    let mut r2_status = "not_configured".to_string();
    let mut r2_client: Option<crate::storage::R2Client> = None;
    if let Some(ref r2_config) = config.r2 {
        if r2_config.enabled {
            match crate::storage::R2Client::from_config(r2_config).await {
                Ok(r2) => {
                    let mut total_ok = 0;
                    let mut total_fail = 0;

                    // Upload MDX content directories
                    if let Some(ref mdx_out) = config.output.mdx_dir {
                        let mdx_path = PathBuf::from(mdx_out);
                        for prefix in &[
                            "daily",
                            "thesis",
                            "assessment",
                            "research",
                            "investigation",
                            "reflection",
                            "decision",
                        ] {
                            // Upload with trailing slash so keys are like "daily/file.md", not "dailyfile.md"
                            let result = r2
                                .upload_dir(&mdx_path, &format!("{}/", prefix), "md")
                                .await;
                            total_ok += result.uploaded.len();
                            total_fail += result.failed.len();
                            for (key, err) in &result.failed {
                                log::warn!("☁️ R2 upload failed [{}]: {}", key, err);
                            }
                        }
                    }

                    // Upload intel JSON files (Layer 2)
                    for intel_path in &bundle.intel_paths {
                        if intel_path.exists() {
                            if let Ok(data) = std::fs::read(intel_path) {
                                let r2_key = format!(
                                    "intel/daily/{}",
                                    intel_path.file_name().unwrap_or_default().to_string_lossy()
                                );
                                if let Err(e) = r2.upload(&r2_key, &data, "application/json").await
                                {
                                    log::warn!("☁️ R2 intel upload failed [{}]: {}", r2_key, e);
                                    total_fail += 1;
                                } else {
                                    total_ok += 1;
                                }
                            }
                        }
                    }

                    // Upload manifest — try mdx_dir first, fall back to frontend_public_dir
                    let manifest_sources = [
                        config.output.mdx_dir.as_deref(),
                        config.output.frontend_public_dir.as_deref(),
                    ];
                    for manifest_dir in manifest_sources.iter().flatten() {
                        let manifest_path = PathBuf::from(manifest_dir).join("manifest.json");
                        if let Ok(data) = std::fs::read(&manifest_path) {
                            if let Err(e) = r2.upload_json("manifest.json", &data).await {
                                log::warn!("⚠️ R2 manifest.json upload failed: {}", e);
                            }
                            break;
                        }
                    }

                    // Upload state files for CI persistence (memory_db, registries, entity_db)
                    // Note: adding a new state file? Add it here AND ensure it's
                    // included in CI's aws s3 sync (cron_brief.yml "Pull persistent state" step).
                    let state_files = [
                        (config.output.vault_path.as_str(), "memory_db.json"),
                        (config.output.vault_path.as_str(), "decision_registry.json"),
                        (
                            config.output.vault_path.as_str(),
                            "assessment_registry.json",
                        ),
                        (
                            config.output.vault_path.as_str(),
                            "investigation_registry.json",
                        ),
                    ];
                    for (base_dir, filename) in &state_files {
                        let state_path = PathBuf::from(base_dir).join(filename);
                        if let Ok(data) = std::fs::read(&state_path) {
                            let r2_key = format!("state/{}", filename);
                            if let Err(e) = r2.upload_json(&r2_key, &data).await {
                                log::warn!("⚠️ R2 state/{} upload failed: {}", filename, e);
                            } else {
                                log::debug!("☁️ R2 state/{} uploaded", filename);
                            }
                        }
                    }
                    let entity_path = data_dir.join("entity_db.json");
                    if entity_path.exists() {
                        if let Ok(data) = std::fs::read(&entity_path) {
                            if let Err(e) = r2.upload_json("state/entity_db.json", &data).await {
                                log::warn!("⚠️ R2 state/entity_db.json upload failed: {}", e);
                            }
                        }
                    }

                    // intel.db (SQLite — checkpointed before main exits)
                    let intel_db_path = data_dir.join("intel.db");
                    if intel_db_path.exists() {
                        if let Ok(data) = std::fs::read(&intel_db_path) {
                            if let Err(e) = r2
                                .upload("state/intel.db", &data, "application/octet-stream")
                                .await
                            {
                                log::warn!("⚠️ R2 state/intel.db upload failed: {}", e);
                            }
                        }
                    }
                    // database.json (ChronicleDb — Hermes dependency)
                    let db_path = data_dir.join("database.json");
                    if db_path.exists() {
                        if let Ok(data) = std::fs::read(&db_path) {
                            if let Err(e) = r2.upload_json("state/database.json", &data).await {
                                log::warn!("⚠️ R2 state/database.json upload failed: {}", e);
                            }
                        }
                    }
                    // event_log.json (EventLog — reset between CI runs)
                    let el_path = data_dir.join("event_log.json");
                    if el_path.exists() {
                        if let Ok(data) = std::fs::read(&el_path) {
                            if let Err(e) = r2.upload_json("state/event_log.json", &data).await {
                                log::warn!("⚠️ R2 state/event_log.json upload failed: {}", e);
                            }
                        }
                    }
                    if total_fail > 0 {
                        r2_status =
                            format!("partial_failure ({}/{})", total_ok, total_ok + total_fail);
                    } else if total_ok > 0 {
                        r2_status = format!("ok ({} files)", total_ok);
                    } else {
                        r2_status = "no_files".to_string();
                    }
                    log::info!("☁️ R2 upload complete: {}", r2_status);
                    r2_client = Some(r2);
                }
                Err(e) => {
                    r2_status = format!("failed: {}", e);
                    log::warn!("⚠️ R2 client init failed: {}", e);
                }
            }
        }
    }

    // 6. Collect all events: pipeline events + publisher events
    let pipeline_event_count = artifacts.events.len();
    let mut all_events: Vec<ObjectEvent> = artifacts.events;

    // Add publish_rejected events for rejected objects（带真实 object_id）
    for result in &report.results {
        if !result.passed {
            all_events.push(ObjectEvent::new(
                ObjectEventType::PublishRejected,
                &result.object_id,
                &result.object_type,
                serde_json::json!({
                    "errors": result.errors,
                    "warnings": result.warnings,
                }),
                "delivery_publisher",
            ));
        }
    }

    // Add publish_completed event (summary anchor for the JSONL)
    all_events.push(ObjectEvent::complete(
        "delivery_publisher",
        serde_json::json!({
            "passed": passed,
            "rejected": rejected,
            "r2_status": r2_status,
        }),
    ));

    // 7. Flush all events to data/events/{date}.jsonl
    if !all_events.is_empty() {
        let events_dir = data_dir.join("events");
        std::fs::create_dir_all(&events_dir)?;
        let events_path = events_dir.join(format!("{}.jsonl", today));
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events_path)?;
        use std::io::Write;
        for event in &all_events {
            let line = serde_json::to_string(event)?;
            writeln!(file, "{}", line)?;
        }
        log::info!(
            "📋 Events flushed: {} events ({} pipeline + {} publisher) to {}",
            all_events.len(),
            pipeline_event_count,
            all_events.len() - pipeline_event_count,
            events_path.display()
        );

        // 7b. 补传 events 到 R2（flush 后才能上传，确保 publish_completed 在档）
        // events 上传失败不回写 events（自引用循环），以 CI log 为准
        if let Some(r2) = &r2_client {
            if let Ok(data) = std::fs::read(&events_path) {
                if let Err(e) = r2.upload(&format!("events/{}.jsonl", today), &data, "application/json").await
                {
                    log::warn!("⚠️ R2 events/{}.jsonl upload failed: {}", today, e);
                }
            }
        }
    }

    Ok(PublishReport {
        passed_count: passed,
        rejected_count: rejected,
        r2_status,
        manifest_version: manifest.version,
        translation_coverage: artifacts.translation_coverage,
    })
}
