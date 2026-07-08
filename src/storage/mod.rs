//! 持久化辅助工具 + R2 云存储客户端
//!
//! - `with_corrupt_recovery()` — JSON 文件损坏备份恢复
//! - `R2Client` — Cloudflare R2 对象存储上传

pub mod r2;
pub use r2::R2Client;

use std::path::Path;

/// load_or_new + 损坏备份保护的便捷实现
///
/// 如果文件不存在，直接返回 fallback()。
/// 如果加载失败，备份损坏文件（加 `.corrupt.{timestamp}` 后缀）并返回 fallback()。
///
/// 预期使用模式:
/// ```ignore
/// let db = with_corrupt_recovery(path, |p| Db::load(p), Db::new);
/// ```
pub fn with_corrupt_recovery<T>(
    path: &Path,
    load_fn: impl FnOnce(&Path) -> anyhow::Result<T>,
    fallback: impl FnOnce() -> T,
) -> T {
    if !path.exists() {
        return fallback();
    }
    match load_fn(path) {
        Ok(value) => value,
        Err(e) => {
            let backup = format!(
                "{}.corrupt.{}",
                path.to_string_lossy(),
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            );
            log::warn!(
                "⚠️ 持久化数据损坏 ({}), 备份到 {} 后重建",
                e,
                backup
            );
            let _ = std::fs::rename(path, &backup);
            fallback()
        }
    }
}
