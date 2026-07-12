//! StepContext — Inteligence Pipeline 步骤间的共享上下文
//!
//! 每个 Step 通过 StepContext 获取共享配置（LLM 客户端、日期、调试模式等）。
//! 不传递所有权，所有字段为只读引用。

use std::path::PathBuf;

/// 步骤上下文 — 传递给每个 Step 的共享环境
#[derive(Debug, Clone)]
pub struct StepContext {
    /// 今日日期（ISO 8601，如 "2026-07-12"）
    pub today: String,
    /// 调试模式标志
    pub debug: bool,
    /// Debug JSON 输出目录（当 debug = true 时有效）
    pub debug_dir: Option<PathBuf>,
}

impl StepContext {
    /// 创建生产模式上下文（零 IO）
    pub fn new(today: &str) -> Self {
        Self {
            today: today.to_string(),
            debug: false,
            debug_dir: None,
        }
    }

    /// 创建调试模式上下文（每步写 JSON 到 debug_dir）
    pub fn new_debug(today: &str, debug_dir: PathBuf) -> Self {
        Self {
            today: today.to_string(),
            debug: true,
            debug_dir: Some(debug_dir),
        }
    }

    /// 是否需要将步骤输出写入文件
    pub fn should_write_debug(&self) -> bool {
        self.debug && self.debug_dir.is_some()
    }
}

/// 调试配置（config.toml 对应字段）
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DebugConfig {
    /// 是否启用调试模式
    #[serde(default)]
    pub enabled: bool,
    /// Debug JSON 输出目录，默认 "debug/pipeline/"
    #[serde(default = "default_debug_dir")]
    pub output_dir: String,
}

fn default_debug_dir() -> String {
    "debug/pipeline".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_context_production() {
        let ctx = StepContext::new("2026-07-12");
        assert!(!ctx.debug);
        assert!(ctx.debug_dir.is_none());
        assert_eq!(ctx.today, "2026-07-12");
    }

    #[test]
    fn test_step_context_debug() {
        let ctx = StepContext::new_debug("2026-07-12", PathBuf::from("debug/"));
        assert!(ctx.debug);
        assert!(ctx.should_write_debug());
    }

    #[test]
    fn test_debug_config_defaults() {
        let config: DebugConfig = toml::from_str("").unwrap();
        assert!(!config.enabled);
        assert_eq!(config.output_dir, "debug/pipeline");
    }
}
