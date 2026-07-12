//! 实体提取 — 从文本中识别技术/组织实体
//!
//! 使用关键词模式匹配从原始文本中提取标准化的实体名称。
//! 非 AI 驱动，纯规则。

/// 实体归一化映射 — 将原始文本中的技术术语映射到标准化实体名称
///
/// 使用大小写不敏感的关键词匹配提取高价值实体。
/// 实体列表覆盖半导体、AI、云计算等核心跟踪领域。
pub fn extract_entities_from_text(text: &str) -> Vec<String> {
    let mut entities = Vec::new();
    let lower = text.to_lowercase();

    let patterns: Vec<(&str, &[&str])> = vec![
        ("TSMC", &["tsmc", "taiwan semiconductor", "台积电"]),
        (
            "ASML",
            &["asml", "advanced semiconductor materials lithography"],
        ),
        ("NVIDIA", &["nvidia", "nvidia corporation", "英伟达"]),
        ("Intel", &["intel", "intel corporation", "英特尔"]),
        ("AMD", &["amd", "advanced micro devices"]),
        ("Samsung", &["samsung", "samsung electronics", "三星"]),
        ("Microsoft", &["microsoft", "msft", "微软"]),
        ("Google", &["google", "alphabet", "谷歌"]),
        ("Meta", &["meta", "facebook", "meta platforms"]),
        ("Amazon", &["amazon", "amzn", "aws", "亚马逊"]),
        ("HBM", &["hbm", "high-bandwidth memory", "hbm3", "hbm4"]),
        ("RISC-V", &["risc-v", "riscv", "open source isa"]),
        ("CUDA", &["cuda", "nvidia cuda", "cuda ecosystem"]),
        ("ARM", &["arm", "arm architecture", "arm holdings"]),
        ("OpenAI", &["openai", "open ai", "chatgpt", "gpt"]),
        ("Anthropic", &["anthropic", "claude"]),
        ("DeepSeek", &["deepseek", "深度求索"]),
    ];

    for (name, aliases) in patterns {
        if aliases.iter().any(|a| lower.contains(a)) && !entities.contains(&name.to_string()) {
            entities.push(name.to_string());
        }
    }

    entities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_entities_basic() {
        let text = "TSMC announced 3nm mass production, NVIDIA's CUDA ecosystem grows";
        let entities = extract_entities_from_text(text);
        assert!(entities.contains(&"TSMC".to_string()));
        assert!(entities.contains(&"NVIDIA".to_string()));
        assert!(entities.contains(&"CUDA".to_string()));
        assert_eq!(entities.len(), 3);
    }

    #[test]
    fn test_extract_entities_empty() {
        let entities = extract_entities_from_text("nothing to see here");
        assert!(entities.is_empty());
    }
}
