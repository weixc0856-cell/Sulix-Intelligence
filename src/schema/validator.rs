//! 通用验证器 — 对 Schema 对象执行批量验证 + 生成验证报告

use std::path::Path;
use serde::{Deserialize, Serialize};

/// 验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub object_id: String,
    pub object_type: String,
    pub passed: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// 批量验证报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub pipeline_date: String,
    pub total_objects: usize,
    pub passed: usize,
    pub failed: usize,
    pub warnings: usize,
    pub results: Vec<ValidationResult>,
}

impl ValidationReport {
    pub fn new(date: &str) -> Self {
        Self {
            pipeline_date: date.to_string(),
            total_objects: 0,
            passed: 0,
            failed: 0,
            warnings: 0,
            results: Vec::new(),
        }
    }

    pub fn add_result(&mut self, result: ValidationResult) {
        let has_errors = !result.errors.is_empty();
        let has_warnings = !result.warnings.is_empty();

        self.total_objects += 1;
        if has_errors {
            self.failed += 1;
        } else {
            self.passed += 1;
        }
        if has_warnings {
            self.warnings += 1;
        }
        self.results.push(result);
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        log::info!("📋 Validation report: {}/{} passed ({} warnings)",
            self.passed, self.total_objects, self.warnings);
        Ok(())
    }

    pub fn has_failures(&self) -> bool {
        self.failed > 0
    }
}

/// 验证器 trait
pub trait Validate {
    fn object_type() -> &'static str;
    fn object_id(&self) -> &str;
    fn validate(&self) -> Vec<String>;
    fn warnings(&self) -> Vec<String> { vec![] }

    fn check(&self) -> ValidationResult {
        let errors = self.validate();
        let warnings = self.warnings();
        ValidationResult {
            object_id: self.object_id().to_string(),
            object_type: Self::object_type().to_string(),
            passed: errors.is_empty(),
            errors,
            warnings,
        }
    }
}
