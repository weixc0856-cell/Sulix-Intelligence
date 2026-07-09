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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_with_corrupt_recovery_missing_file() {
        let path = Path::new("nonexistent_file_for_test.json");
        // When file doesn't exist, fallback is called (not the load function)
        let result: i32 = with_corrupt_recovery(path, |_| Ok(42), || 0);
        assert_eq!(result, 0); // fallback returns 0 for missing file
    }

    #[test]
    fn test_with_corrupt_recovery_valid_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_valid_recovery.json");
        fs::write(&path, "{\"key\": \"value\"}").unwrap();
        let result: serde_json::Value = with_corrupt_recovery(
            &path,
            |p| Ok(serde_json::from_str::<serde_json::Value>(&fs::read_to_string(p).unwrap()).unwrap()),
            || serde_json::json!({}),
        );
        assert_eq!(result["key"], "value");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_with_corrupt_recovery_corrupt_file_fallback() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_corrupt_recovery.json");
        fs::write(&path, "not valid json").unwrap();
        let result: i32 = with_corrupt_recovery(
            &path,
            |p| { let _ = fs::read_to_string(p)?; anyhow::bail!("simulated parse error"); },
            || 99,
        );
        assert_eq!(result, 99);
        // cleanup: corrupt backup file
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name().to_string_lossy().contains("test_corrupt_recovery.json.corrupt.") {
                let _ = fs::remove_file(entry.path());
            }
        }
        let _ = fs::remove_file(&path);
    }
}
