//! Investigation Engine — Thesis → 结构化问题生成
//!
//! v1 约束:
//!   - 仅在新 thesis 创建时生成一次
//!   - ≤5 questions
//!   - 1 falsification condition max per question
//!   - 无证据匹配（v2）

use anyhow::Result;

use crate::config::LlmConfig;
use crate::domain::investigation::{Investigation, InvestigationReport, Question, QuestionStatus};
use crate::domain::thesis::Thesis;

const SYSTEM_PROMPT: &str = r#"You are a strategic analyst. Your job is to decompose a strategic judgment into 3-5 key questions that need answering to validate or invalidate it.

For each question, provide:
1. The question itself (in Chinese, actionable)
2. Importance (1-10): how critical this question is to the thesis
3. A testable hypothesis (what you expect to find if the thesis is correct)
4. A falsification condition (what would prove this wrong, maximum 1)

Rules:
- Output exactly 3-5 questions. No more than 5.
- Each question must be falsifiable.
- No philosophical debates. Each question must have an observable answer.
- Focus on strategic/commercial implications, not academic interest.

Output ONLY valid JSON, no markdown fences:
{
  "questions": [
    {
      "text": "用户是否真的愿意为 Agent 付费？",
      "importance": 9,
      "hypothesis": "用户可接受 $20/mo 的 Agent 订阅",
      "falsification": "获客成本 > 3x LTV"
    }
  ]
}"#;

/// 为 Thesis 生成 Investigation（结构化问题集）
///
/// 仅在 thesis 状态 >= Active 且尚无 investigation 时调用。
/// 调用一次后不重新生成（v1 约束）。
pub async fn generate_investigation(
    thesis: &Thesis,
    api_key: &str,
    llm_config: &LlmConfig,
    prompts: Option<&crate::config::PromptsConfig>,
) -> Result<Investigation> {
    let user_prompt = format!(
        "Thesis: {}\n\nBLUF: {}\n\n背景:\n{}\n\n请生成 3-5 个关键问题来验证或证伪这个判断。",
        thesis.title,
        thesis
            .evidences
            .last()
            .map(|e| e.summary.as_str())
            .unwrap_or(""),
        thesis
            .assumptions
            .iter()
            .map(|a| format!(
                "- {} (承重: {}, 强度: {})",
                a.text, a.load_bearing, a.evidence_strength
            ))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    let client = crate::client::global_client();
    // v1: use hardcoded prompt, no config override
    let _ = prompts; // suppress unused
    let system_prompt = SYSTEM_PROMPT;

    let raw =
        crate::llm::call_with_retry_raw(client, api_key, llm_config, system_prompt, &user_prompt)
            .await?;
    let parsed = crate::llm::parse_json_lenient(&raw)?;

    let questions_array = parsed["questions"].as_array().ok_or_else(|| {
        anyhow::anyhow!("LLM returned invalid investigation JSON: missing 'questions' array")
    })?;

    let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut questions = Vec::new();

    for (i, q) in questions_array.iter().enumerate() {
        if i >= 5 {
            break; // v1: ≤5 questions
        }
        let text = q["text"].as_str().unwrap_or("Unknown question").to_string();
        let importance = q["importance"].as_u64().unwrap_or(5).min(10) as u8;
        let hypothesis = q["hypothesis"].as_str().map(|s| s.to_string());
        let falsification = q["falsification"].as_str().map(|s| s.to_string());

        questions.push(Question {
            id: format!("q-{}-{}", thesis.id, i),
            text,
            importance,
            hypothesis,
            falsification,
            status: QuestionStatus::Unanswered,
            answers: vec![],
            created_at: now.clone(),
            updated_at: now.clone(),
        });
    }

    if questions.is_empty() {
        anyhow::bail!(
            "LLM returned no valid questions for thesis '{}'",
            thesis.title
        );
    }

    Ok(Investigation {
        id: format!("inv-{}", chrono::Utc::now().timestamp()), // replaced with INV-XXXX in publishing.rs
        thesis_id: thesis.id.clone(),
        generated_at: now,
        questions,
        state: "active".to_string(),
        primary_domain: crate::domain::StrategicDomain::default(),
        secondary_domains: vec![],
    })
}

/// Thesis 数据派生 Investigation Report（无 LLM 调用）
///
/// 直接从 Thesis 的已有数据（evidences, assumptions, falsification_conditions）
/// 构造结构化调查报告，供 MDX 渲染和前端 Investigation 页面使用。
pub fn derive_investigation_report(
    thesis: &crate::domain::thesis::Thesis,
    today: &str,
    decision_rationale: Option<&str>,
) -> InvestigationReport {
    use crate::domain::evidence::Stance;

    let core_question = format!(
        "Will the following assessment hold true? \"{}\"",
        thesis.title
    );

    // 支持证据（最多 5 条）
    let supporting_evidence: Vec<String> = thesis
        .evidences
        .iter()
        .filter(|e| e.stance == Stance::Supports)
        .take(5)
        .map(|e| format!("[{}] {}", e.source, e.summary))
        .collect();

    // 反对证据（最多 5 条）
    let counter_evidence: Vec<String> = thesis
        .evidences
        .iter()
        .filter(|e| e.stance == Stance::Challenges)
        .take(5)
        .map(|e| format!("[{}] {}", e.source, e.summary))
        .collect();

    // 关键未知：承重假设中证据弱的条目
    let key_unknowns: Vec<String> = thesis
        .assumptions
        .iter()
        .filter(|a| a.load_bearing && a.evidence_strength == "weak")
        .take(4)
        .map(|a| format!("Validate: {}", a.text))
        .collect();

    // 证伪条件（已有字段）
    let falsification_conditions = thesis.falsification_conditions.clone();

    // 初步结论：优先用 decision_rationale，否则用最新证据摘要
    let preliminary_conclusion = decision_rationale
        .map(|s| s.to_string())
        .or_else(|| thesis.evidences.last().map(|e| e.summary.clone()))
        .unwrap_or_else(|| format!("Assessment '{}' is currently being tracked.", thesis.title));

    // Derive status from thesis status
    let status = match thesis.status {
        crate::domain::thesis::ThesisStatus::Dormant
        | crate::domain::thesis::ThesisStatus::Retired => "archived",
        _ => "active",
    };

    InvestigationReport {
        thesis_id: thesis.id.clone(),
        thesis_title: thesis.title.clone(),
        date: today.to_string(),
        core_question,
        supporting_evidence,
        counter_evidence,
        key_unknowns,
        falsification_conditions,
        preliminary_conclusion,
        status: status.to_string(),
        primary_domain: thesis.primary_domain,
        secondary_domains: thesis.secondary_domains.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_question_count_limit() {
        // Without LLM, test the parsing logic with mock JSON
        let json = serde_json::json!({
            "questions": (0..10).map(|i| serde_json::json!({
                "text": format!("Question {}", i),
                "importance": 5,
                "hypothesis": "test",
                "falsification": "test"
            })).collect::<Vec<_>>()
        });

        let now = "2026-06-25";
        let mut questions = Vec::new();
        if let Some(arr) = json["questions"].as_array() {
            for (i, q) in arr.iter().enumerate() {
                if i >= 5 {
                    break;
                }
                questions.push(Question {
                    id: format!("q-t1-{}", i),
                    text: q["text"].as_str().unwrap_or("").to_string(),
                    importance: q["importance"].as_u64().unwrap_or(5).min(10) as u8,
                    hypothesis: None,
                    falsification: None,
                    status: QuestionStatus::Unanswered,
                    answers: vec![],
                    created_at: now.to_string(),
                    updated_at: now.to_string(),
                });
            }
        }
        assert_eq!(questions.len(), 5, "should cap at 5 questions");
    }

    #[test]
    fn test_minimum_questions() {
        let json = serde_json::json!({"questions": []});
        let questions_array = json["questions"].as_array().unwrap();
        assert!(
            questions_array.is_empty(),
            "empty array should yield 0 questions"
        );
    }
}
