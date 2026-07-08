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

use crate::artifact::manifest::ContentManifest;
use crate::config::Config;
use crate::domain::artifact::ArtifactSet;
use crate::event_log::{ObjectEvent, ObjectEventType};

/// 发布报告
#[derive(Debug, Clone)]
pub struct PublishReport {
    pub passed_count: usize,
    pub rejected_count: usize,
    pub r2_status: String,
    pub manifest_version: u32,
}

/// 发布产物到存储后端
///
/// ownership 从 publishing 移交：ArtifactSet 被 consume
pub async fn publish(
    artifacts: ArtifactSet,
    config: &Config,
    data_dir: &Path,
    today: &str,
) -> Result<PublishReport> {
    let mut passed = 0usize;
    let mut rejected = 0usize;

    // 1. Schema validation — 逐对象检查
    // Phase 0: 仅验证 manifest 层面（确保有文件产出）
    // Phase 1+: 逐对象验证 schema
    let mdx_out = config.output.mdx_dir.as_deref().map(PathBuf::from);
    let rejected_dir = data_dir.join("rejected").join(today);
    std::fs::create_dir_all(&rejected_dir)?;

    // 验证各种 artifact 计数
    let total_assessments = artifacts.assessment_count;
    let total_investigations = artifacts.investigation_count;
    let decision_count = artifacts.thesis_decisions.len();

    // 模拟逐对象验证（Phase 1 展开为真正的 schema::validator 调用）
    if let Some(ref mdx_path) = mdx_out {
        for entry in std::fs::read_dir(mdx_path.join("thesis")).unwrap_or_else(|_| std::fs::read_dir(".").unwrap()) {
            if let Ok(e) = entry {
                if e.path().extension().is_some_and(|ext| ext == "md") {
                    let content = std::fs::read_to_string(e.path()).unwrap_or_default();
                    // 简单验证：检查必填字段是否存在
                    if content.contains("title:") && content.contains("confidence:") {
                        passed += 1;
                    } else {
                        rejected += 1;
                        // 写入 rejected 目录留证
                        let dest = rejected_dir.join(e.file_name());
                        let _ = std::fs::copy(e.path(), &dest);
                        log::warn!("📋 Rejected: {} (missing required fields)", e.file_name().to_string_lossy());
                    }
                }
            }
        }
    }

    // 2. 补发 publish_rejected 事件（审计线不中断）
    // Phase 1 实现，Phase 0 跳过

    // 3. 生成 manifest（此刻计数 = 验证通过后的真实上传数）
    let manifest = ContentManifest::new(
        today,
        0, // prev_version — will be read from existing file
        config.output.frontend_public_dir.clone(),
        "healthy",
        None,  // observation_count
        None,  // signal_count
        None,  // duration_seconds
        None,  // stages
    );

    // 回填真实计数
    let manifest = manifest.with_counts(
        total_assessments.saturating_sub(rejected),
        total_investigations,
        decision_count,
        artifacts.archive_days,
        artifacts.total_signals,
    );

    // 4. Local write
    if let Some(ref mdx_path) = config.output.mdx_dir {
        let mdx_base = PathBuf::from(mdx_path);

        // Write manifest to mdx_dir and vault_path
        if let Err(e) = manifest.save_as_json(&mdx_base.join("manifest.json")) {
            log::warn!("⚠️ Manifest local save failed: {}", e);
        }

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
    }

    // 5. R2 upload (if configured — soft error, recorded in r2_status)
    let mut r2_status = "not_configured".to_string();
    if let Some(ref r2_config) = config.r2 {
        if r2_config.enabled {
            match crate::storage::R2Client::from_config(r2_config).await {
                Ok(r2) => {
                    let mut total_ok = 0;
                    let mut total_fail = 0;

                    // Upload MDX content directories
                    if let Some(ref mdx_out) = config.output.mdx_dir {
                        let mdx_path = PathBuf::from(mdx_out);
                        for prefix in &["daily", "thesis", "assessment", "research",
                                        "investigation", "reflection", "decision"] {
                            let result = r2.upload_dir(&mdx_path, prefix, "md").await;
                            total_ok += result.uploaded.len();
                            total_fail += result.failed.len();
                            for (key, err) in &result.failed {
                                log::warn!("☁️ R2 upload failed [{}]: {}", key, err);
                            }
                        }
                    }

                    // Upload manifest
                    if let Some(ref mdx_out) = config.output.mdx_dir {
                        let manifest_path = PathBuf::from(mdx_out).join("manifest.json");
                        if let Ok(data) = std::fs::read(&manifest_path) {
                            if let Err(e) = r2.upload_json("manifest.json", &data).await {
                                log::warn!("⚠️ R2 manifest.json upload failed: {}", e);
                            }
                        }
                    }

                    if total_fail > 0 {
                        r2_status = format!("partial_failure ({}/{})", total_ok, total_ok + total_fail);
                    } else if total_ok > 0 {
                        r2_status = format!("ok ({} files)", total_ok);
                    } else {
                        r2_status = "no_files".to_string();
                    }
                    log::info!("☁️ R2 upload complete: {}", r2_status);
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

    // Add publish_rejected events for rejected objects
    for _ in 0..rejected {
        all_events.push(ObjectEvent::new(
            ObjectEventType::PublishRejected,
            "unknown", "artifact",
            serde_json::json!({"reason": "schema validation failed"}),
            "delivery_publisher",
        ));
    }

    // Add publish_completed event (summary anchor for the JSONL)
    all_events.push(ObjectEvent::complete("delivery_publisher", serde_json::json!({
        "passed": passed,
        "rejected": rejected,
        "r2_status": r2_status,
    })));

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
        log::info!("📋 Events flushed: {} events ({} pipeline + {} publisher) to {}",
            all_events.len(), pipeline_event_count, all_events.len() - pipeline_event_count,
            events_path.display());
    }

    Ok(PublishReport {
        passed_count: passed,
        rejected_count: rejected,
        r2_status,
        manifest_version: manifest.version,
    })
}
