//! Storage trait — 统一持久化接口（设计存档）
//!
//! 本模块的 `Saveable` / `Loadable` trait 最初是为消除 7 个模块中
//! 复制粘贴的 save/load 逻辑而设计（Phase 3 蓝图）。
//!
//! 实际演进中，`with_corrupt_recovery()` 函数模式取代了 trait 方案——
//! 各模块保持各自的持久化签名，通过高阶函数获得损坏备份保护。
//! 此 trait 设计已归档，不再推进实现。

use std::path::Path;

/// 可持久化的数据模型
///
/// **设计存档**：已被 `with_corrupt_recovery()` + 模块自有 save 签名取代。
#[doc(hidden)]
pub trait Saveable {
    /// 序列化并写入 JSON 文件
    fn save_to_json(&self, path: &Path) -> anyhow::Result<()>;
}

/// 可从磁盘加载的数据模型（含损坏备份保护）
///
/// **设计存档**：已被 `with_corrupt_recovery()` + 模块自有 load 签名取代。
#[doc(hidden)]
pub trait Loadable: Sized {
    /// 从文件加载；若文件不存在或损坏返回 None
    fn load_from_json(path: &Path) -> anyhow::Result<Option<Self>>;
}

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
