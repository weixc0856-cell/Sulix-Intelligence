//! DataCatalog — 认知审计链（Cognitive Audit Trail）
//!
//! 每层中间状态按步骤 JSON 落盘，trace_id (article.id) 贯穿全链路。
//! 示例: data/2026-06-21/01_raw_signals.json
//!
//! 出问题时可用 grep trace_id data/2026-06-21/* 定位全生命周期。

use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// 每层落盘，JSON 文件存储
pub struct DataCatalog {
    step_dir: PathBuf,
}

impl DataCatalog {
    /// 创建目录 {base_dir}/{date}/
    pub fn new(base_dir: &Path, date: &str) -> Self {
        let step_dir = base_dir.join(date);
        fs::create_dir_all(&step_dir).ok();
        log::info!("📂 认知审计链: {}", step_dir.display());
        Self { step_dir }
    }

    /// 保存步骤输出
    ///
    /// save_step(1, "raw_signals", &articles) → data/2026-06-21/01_raw_signals.json
    pub fn save_step<T: Serialize>(&self, index: u32, name: &str, data: &T) -> Result<()> {
        let path = self.step_dir.join(format!("{:02}_{}.json", index, name));
        let json = serde_json::to_string_pretty(data)?;
        fs::write(&path, &json)?;
        log::debug!(
            "  🪵 已落盘: {}",
            path.file_name().map(|f| f.to_string_lossy()).unwrap_or_else(|| std::borrow::Cow::Borrowed("(unknown)"))
        );
        Ok(())
    }
}
