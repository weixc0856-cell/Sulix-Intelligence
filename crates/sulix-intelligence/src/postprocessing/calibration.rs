//! Calibration — 认知校准（扎心问题）
//!
//! 在日报底部留 1 个扎心问题，主动探测认知边界。
//! 「不改也行，看到就有收获。」
//!
//! 旧版: agent/calibration.rs（接收旧 VerticalAnalysis）
//! 新版: 接收 pipeline 输出的 signals + theses + decisions
//!
//! 与旧版的行为差异：
//!   旧版输入是 VerticalAnalysis（分析后的文章）
//!   新版输入是 contract::Signal + contract::Thesis（结构化判断）
//!   LLM prompt 相同，输入格式不同

use sulix_config::LlmConfig;
use sulix_contract as contract;
use sulix_llm as llm;

/// 生成校准问题（1 个扎心提问）
///
/// 输入今日管线输出的信号 + 判断 + 决策，输出一个问题字符串。
/// 失败时返回空字符串，调用方忽略即可。
pub async fn calibrate(
    signals: &[contract::Signal],
    theses: &[contract::Thesis],
    decisions: &[contract::Decision],
    llm_config: &LlmConfig,
    api_key: &str,
    language: &str,
) -> String {
    if signals.is_empty() && theses.is_empty() {
        return String::new();
    }

    let client = match llm::create_client(60) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("⚠️ Calibration: 创建 client 失败: {}", e);
            return String::new();
        }
    };

    let system_prompt = if language == "en" {
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

    // 构建输入上下文
    let mut context = String::from("今日信号:\n");
    for s in signals.iter().take(10) {
        context.push_str(&format!(
            "  [{:?}] {} (domain={}, imp={:.2})\n",
            s.category, s.why, s.domain, s.importance
        ));
    }

    if !theses.is_empty() {
        context.push_str("\n今日判断:\n");
        for t in theses.iter().take(5) {
            context.push_str(&format!(
                "  [{}] {} (conf={:.2}, status={:?})\n",
                t.id, t.claim, t.confidence, t.status
            ));
        }
    }

    if !decisions.is_empty() {
        context.push_str("\n今日决策:\n");
        for d in decisions.iter().take(5) {
            context.push_str(&format!("  {}: {:?}\n", d.thesis_id, d.action));
        }
    }

    let user_prompt = format!("今天的分析结果如下，请生成校准问题：\n{}", context);

    match llm::call_with_retry(&client, api_key, llm_config, system_prompt, &user_prompt).await {
        Ok(results) => {
            let question = results
                .first()
                .map(|r| r.judgment.clone())
                .unwrap_or_default();
            if !question.is_empty() {
                log::info!(
                    "🤖 Calibration: {}",
                    &question[..question.floor_char_boundary(80)]
                );
            }
            question
        }
        Err(e) => {
            log::warn!("⚠️ Calibration failed: {}", e);
            String::new()
        }
    }
}
