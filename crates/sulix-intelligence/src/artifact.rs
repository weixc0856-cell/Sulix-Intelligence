//! Artifact — 管线步骤间的类型安全中间表示
//!
//! Artifact 是 Intelligence Pipeline 中步骤间传递的唯一数据类型。
//! 每个步骤消费一种 Artifact variant，生产另一种。
//! 编译期保证不会出现"步骤接收到错误类型"的情况。
//!
//! Production 模式：内存中 Artifact enum 流转，零 IO。
//! Debug 模式：Artifact 序列化为 JSON 文件，可独立重放。

use anyhow::Result;

use sulix_contract as contract;

/// 管线步骤间的类型安全契约
///
/// # Type-level safety
/// 每个 accessor (into_observations 等) 在错误 variant 上返回错误。
/// 比 `Vec<u8>` 安全（编译期类型检查），
/// 比 `Box<dyn Any>` 简单（无需 downcast）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Artifact {
    /// 原始观察（RSS/源适配器输出）
    Observations(Vec<contract::Observation>),
    /// 分类后的信号
    Signals(Vec<contract::Signal>),
    /// 可追踪判断
    Theses(Vec<contract::Thesis>),
    /// 行动决策
    Decisions(Vec<contract::Decision>),
}

impl Artifact {
    /// 提取 Observations — 在错误 variant 上返回错误
    pub fn into_observations(self) -> Result<Vec<contract::Observation>> {
        match self {
            Self::Observations(v) => Ok(v),
            other => Err(anyhow::anyhow!(
                "Artifact type mismatch: expected Observations, got {:?}",
                other.variant_name()
            )),
        }
    }

    /// 提取 Signals — 在错误 variant 上返回错误
    pub fn into_signals(self) -> Result<Vec<contract::Signal>> {
        match self {
            Self::Signals(v) => Ok(v),
            other => Err(anyhow::anyhow!(
                "Artifact type mismatch: expected Signals, got {:?}",
                other.variant_name()
            )),
        }
    }

    /// 提取 Theses — 在错误 variant 上返回错误
    pub fn into_theses(self) -> Result<Vec<contract::Thesis>> {
        match self {
            Self::Theses(v) => Ok(v),
            other => Err(anyhow::anyhow!(
                "Artifact type mismatch: expected Theses, got {:?}",
                other.variant_name()
            )),
        }
    }

    /// 提取 Decisions — 在错误 variant 上返回错误
    pub fn into_decisions(self) -> Result<Vec<contract::Decision>> {
        match self {
            Self::Decisions(v) => Ok(v),
            other => Err(anyhow::anyhow!(
                "Artifact type mismatch: expected Decisions, got {:?}",
                other.variant_name()
            )),
        }
    }

    /// JSON round-trip — Debug 模式用
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// 从 JSON 反序列化
    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    /// 变体名称（用于错误消息）
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::Observations(_) => "Observations",
            Self::Signals(_) => "Signals",
            Self::Theses(_) => "Theses",
            Self::Decisions(_) => "Decisions",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_round_trip_observations() {
        let obs = vec![contract::Observation {
            id: "obs_001".into(),
            title: "test".into(),
            source: "test".into(),
            source_id: String::new(),
            url: "https://test.com".into(),
            published_at: "2026-07-12".into(),
            captured_at: "2026-07-12T00:00:00Z".into(),
            content_hash: "abc".into(),
            raw_content: "content".into(),
            entities: vec![],
        }];
        let artifact = Artifact::Observations(obs.clone());
        let json = artifact.to_json().unwrap();
        let restored = Artifact::from_json(&json).unwrap();
        let extracted = restored.into_observations().unwrap();
        assert_eq!(extracted.len(), 1);
        assert_eq!(extracted[0].id, "obs_001");
    }

    #[test]
    fn test_artifact_type_mismatch_error() {
        let artifact = Artifact::Signals(vec![]);
        let result = artifact.into_observations();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expected Observations"));
    }

    #[test]
    fn test_artifact_variant_name() {
        assert_eq!(Artifact::Observations(vec![]).variant_name(), "Observations");
        assert_eq!(Artifact::Signals(vec![]).variant_name(), "Signals");
        assert_eq!(Artifact::Theses(vec![]).variant_name(), "Theses");
        assert_eq!(Artifact::Decisions(vec![]).variant_name(), "Decisions");
    }
}


