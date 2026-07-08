//! PipelineReport 增强
//!
//! report.save() 负责双写 data/ 和 public/。
//! main 只调一次 save，不拆成两个步骤。

use anyhow::Result;
use std::path::Path;

/// 保存 PipelineReport 到 data/ 和 public/（如配置了 frontend 目录）
/// main 中最终只调此函数一次，确保 public/ 拿到的始终是最终版（含 duration 和 status）
pub fn save_report(
    report: &crate::engine::pipeline_health::PipelineReport,
    data_dir: &Path,
    frontend_public_dir: Option<&str>,
) -> Result<()> {
    // 1. Save to data/{date}/pipeline_report.json
    report.save(data_dir).map_err(|e| anyhow::anyhow!("{:#}", e))?;

    // 2. Save to vault_path for local audit
    let vault_path = std::env::var("VAULT_PATH").unwrap_or_else(|_| "./DailyBrief".into());
    let vault_report_path = std::path::PathBuf::from(&vault_path).join("pipeline_report.json");
    if let Err(e) = report.save_as_json(&vault_report_path) {
        log::warn!("⚠️ Pipeline report vault save failed: {}", e);
    }

    // 3. Sync to frontend public/ if configured (final version with duration and status)
    if let Some(fe_public_dir) = frontend_public_dir {
        let fe_path = std::path::PathBuf::from(fe_public_dir).join("pipeline_report.json");
        if let Err(e) = report.save_as_json(&fe_path) {
            log::warn!("⚠️ Pipeline report frontend sync failed: {}", e);
        } else {
            log::info!("📊 Pipeline report synced to frontend public/");
        }
    }

    Ok(())
}
