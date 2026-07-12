//! 层间契约 — Layer Boundary Contracts
//!
//! 这是 Sulix Intelligence 层间通信的"宪法"。
//! 每个 struct 定义了两层之间的稳定契约：
//!
//!   Observation  ←纯事实→  Intelligence   ←判断→  Memory
//!
//! 核心原则：
//! 1. Observation 层不知道 Signal/Theme/Thesis 的存在
//! 2. Intelligence 层不知道 Memory 的存储细节
//! 3. Memory 层不知道 Observation 的源格式
//!
//! 所有 struct 使用 #[derive(JsonSchema)] 确保可以自动生成 JSON Schema 文件。
//! 生成的 JSON Schema 位于 /schemas/ 目录，供前端消费。

pub mod belief;
pub mod decision;
pub mod observation;
pub mod reflection;
pub mod signal;
pub mod theme;
pub mod thesis;

pub use belief::Belief;
pub use decision::{Decision, DecisionHorizon, DecisionType};
pub use observation::Observation;
pub use reflection::{OutcomeVerdict, Reflection};
pub use signal::{Signal, SignalCategory};
pub use theme::{Theme, ThemeStatus};
pub use thesis::{Thesis, ThesisStatus};

// ===== 边界合规测试 =====

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn observation_is_pure_fact() {
        let obs = Observation {
            id: "obs_001".into(),
            title: "test".into(),
            source: "test".into(),
            source_id: String::new(),
            url: "https://test.com".into(),
            published_at: "2026-07-12".into(),
            captured_at: "2026-07-12T00:00:00Z".into(),
            content_hash: "abc123".into(),
            raw_content: "test content".into(),
            entities: vec![],
        };
        // Observation 可以构造且不包含 category/importance/domain/tags
        assert!(!obs.id.is_empty());
        assert!(!obs.raw_content.is_empty());
        assert!(!obs.content_hash.is_empty());
        println!("✓ Observation 是纯事实结构: id/title/source/source_id/url/published_at/captured_at/content_hash/raw_content/entities");
    }
}

/// Schema 生成器测试 — 生成 7 个 JSON Schema 文件到 /schemas/
///
/// 运行方式: cargo test -- generate_schemas -- --nocapture
#[cfg(test)]
mod tests {
    use super::*;

    fn generate_schema<T: schemars::JsonSchema + ?Sized>(
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let schema = schemars::schema_for!(T);
        let json = serde_json::to_string_pretty(&schema)?;

        let schemas_dir = std::path::Path::new("schemas");
        std::fs::create_dir_all(schemas_dir)?;

        let path = schemas_dir.join(format!("{}.schema.json", name));
        std::fs::write(&path, json)?;
        println!("  ✓ {} -> {}", name, path.display());
        Ok(())
    }

    #[test]
    fn generate_schemas() -> Result<(), Box<dyn std::error::Error>> {
        println!("生成层间契约 JSON Schema...");

        generate_schema::<Observation>("observation")?;
        generate_schema::<Signal>("signal")?;
        generate_schema::<Theme>("theme")?;
        generate_schema::<Belief>("belief")?;
        generate_schema::<Thesis>("thesis")?;
        generate_schema::<Decision>("decision")?;
        generate_schema::<Reflection>("reflection")?;

        println!("\n✓ 7 个 JSON Schema 已生成到 schemas/");
        Ok(())
    }
}
