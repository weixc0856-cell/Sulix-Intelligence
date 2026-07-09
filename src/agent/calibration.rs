//! Calibration Agent (🤖 认知校准)
//!
//! 日报底部留 1 个扎心问题，主动探测认知边界。
//! 「不改也行，看到就有收获。」
//!
//! 复用 llm::call_with_retry，纯 prompt 驱动。

use anyhow::Result;

use crate::config::LlmConfig;
use crate::llm;
use crate::llm::VerticalAnalysis;

/// 生成校准问题（1 个扎心提问）
///
/// 输入今天的分析结果，输出一个问题字符串。
/// 失败时返回空字符串，调用方忽略即可。
/// `language` 控制输出语言: "en" 或 "zh"
pub async fn calibrate(
    analysis: &[VerticalAnalysis],
    api_key: &str,
    llm_config: &LlmConfig,
    prompts: Option<&crate::config::PromptsConfig>,
    language: &str,
) -> Result<String> {
    let client = crate::llm::create_llm_client()?;

    let system_prompt_inner = if language == "en" {
        r#"You are a cognitive calibration specialist. Your job is NOT to summarize — it is to ask questions.

Read today's analysis and find the ONE most valuable contradiction, blind spot, or cognitive bias worth challenging.
Output a single sharp question that makes a founder question their own judgment framework.

Rules:
- Output only 1 question, no explanation
- Be specific (reference today's actual content)
- Be sharp but not sarcastic
- Don't ask questions with known answers

Output strict JSON:
{"articles": [{"title": "Cognitive Calibration", "importance": 1, "relevance": "high", "time_horizon": "short", "action": "investigate", "confidence": "medium", "judgment": "Your question here"}]}"#
    } else {
        r#"你是一个认知校准师。你的任务不是总结，而是提问。

阅读今天的分析结果，找出 1 个最值得追问的矛盾、盲点或认知偏见。
输出一个扎心但有用的问题，让创业者反思自己的判断框架。

规则：
- 只输出 1 个问题，不要解释
- 问题要具体（关联今天的具体内容）
- 要扎心但不要阴阳怪气
- 不要问已知答案的问题

Output json. 输出严格 JSON：
{"articles": [{"title": "认知校准", "importance": 1, "relevance": "高", "time_horizon": "短期", "action": "研究", "confidence": "中", "judgment": "你的问题在这里"}]}"#
    };

    let system_prompt = match prompts {
        Some(p) => p.get_calibration(system_prompt_inner),
        None => system_prompt_inner,
    };

    // 将分析结果格式化为简洁的输入
    let user_prompt = format!(
        "今天的分析结果如下，请生成校准问题：\n{}",
        serde_json::to_string_pretty(analysis).unwrap_or_default()
    );

    match llm::call_with_retry(&client, api_key, llm_config, system_prompt, &user_prompt).await {
        Ok(results) => {
            // judgment 字段携带了 LLM 返回的 question 内容
            let question = results
                .first()
                .map(|r| r.judgment.clone())
                .unwrap_or_default();
            if question.is_empty() {
                log::info!("Calibration Agent 未生成问题（返回为空）");
            } else {
                log::info!(
                    "🤖 Calibration: {}",
                    &question[..question.floor_char_boundary(60)]
                );
            }
            Ok(question)
        }
        Err(e) => {
            log::warn!("⚠️ Calibration Agent 失败: {}", e);
            Ok(String::new()) // 失败时返回空字符串，日报不受影响
        }
    }
}
