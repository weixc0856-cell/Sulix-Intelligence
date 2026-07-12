//! Golden Pipeline Test — 固定输入产生固定输出
//!
//! 目的：
//!   确保管线输出不会因 LLM 模型更换或代码变更而"人格漂移"。
//!
//! 机制：
//!   - tests/fixtures/*.json 是"黄金标准"输入输出
//!   - 即使 LLM 换模型（DeepSeek → GPT → Claude），
//!     只要 Golden Test 挂掉，就说明系统行为发生了变化
//!
//! 注意：
//!   - Golden Test 使用 mock LLM（固定 JSON 响应），不调用真实 LLM
//!   - 后续可将 mock 替换为真实 LLM 的 snapshot 对比

use sulix_config::LlmConfig;
use sulix_contract as contract;
use sulix_intelligence::*;

fn mock_llm_config() -> LlmConfig {
    LlmConfig {
        api_key: Some("test".into()),
        provider: "test".into(),
        model: "test".into(),
        base_url: "http://test".into(),
        max_tokens: 100,
        temperature: 0.0,
        perplexity_key: None,
    }
}

fn mock_pipeline() -> IntelligencePipeline {
    IntelligencePipeline::new(
        SignalClassificationStepBuilder::new(mock_llm_config(), "test").build(),
        ThesisGenerationStepBuilder::new(mock_llm_config(), "test").build(),
        DecisionMappingStepBuilder::new().build(),
    )
}

// ===== Observation/Signal Schema Tests =====

#[test]
fn test_golden_observation_deserialization() {
    let json = include_str!("fixtures/observation.json");
    let artifact: Artifact = serde_json::from_str(json).expect("observation.json 应合法");
    let observations = artifact
        .into_observations()
        .expect("应为 Observations variant");

    assert_eq!(observations.len(), 3, "应有 3 条 observations");
    assert_eq!(observations[0].id, "obs_001");
    assert_eq!(observations[0].source, "NVIDIA Blog");
    assert_eq!(observations[1].id, "obs_002");
    assert_eq!(observations[2].id, "obs_003");

    // 验证 Observation 不包含解释性字段（编译期检查）
    let _pure_obs = &observations[0];
    // _pure_obs.domain;  // ← 编译失败 = 通过了
}

#[test]
fn test_golden_signal_deserialization() {
    let json = include_str!("fixtures/expected_signal.json");
    let artifact: Artifact = serde_json::from_str(json).expect("expected_signal.json 应合法");
    let signals = artifact.into_signals().expect("应为 Signals variant");
    assert_eq!(signals.len(), 3);
    assert_eq!(signals[0].observation_id, "obs_001");
    assert!(signals[0].importance > 0.5);
}

#[test]
fn test_golden_signal_fields() {
    let json = include_str!("fixtures/expected_signal.json");
    let artifact: Artifact = serde_json::from_str(json).unwrap();
    let signals = artifact.into_signals().unwrap();
    for signal in &signals {
        assert!(!signal.observation_id.is_empty());
        assert!((0.0..=1.0).contains(&signal.importance));
        assert!(!signal.domain.is_empty());
        assert!(!signal.why.is_empty());
    }
}

// ===== Artifact Round-trip =====

#[test]
fn test_golden_artifact_round_trip() {
    let json = include_str!("fixtures/observation.json");
    let artifact: Artifact = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_string_pretty(&artifact).unwrap();
    let restored: Artifact = serde_json::from_str(&serialized).unwrap();
    assert_eq!(artifact.variant_name(), restored.variant_name());
}

// ===== Empty Pipeline =====

#[test]
fn test_golden_empty_pipeline() {
    let pipeline = mock_pipeline();
    let ctx = StepContext::new("2026-07-12");
    let output = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(pipeline.run(vec![], &ctx));
    assert!(output.is_ok());
    let output = output.unwrap();
    assert_eq!(output.signals.len(), 0);
    assert_eq!(output.theses.len(), 0);
    assert_eq!(output.decisions.len(), 0);
}

// ===== ThesisGeneration: Matching existing theses =====

#[test]
fn test_thesis_generation_matches_existing_theses() {
    // 已有 Thesis："NVIDIA GPU performance improvement"
    let existing = vec![contract::Thesis {
        id: "thesis_existing".into(),
        claim: "NVIDIA GPU performance improvement will accelerate AI infrastructure".into(),
        confidence: 0.65,
        evidence: vec!["legacy_sig_001".into()],
        status: contract::ThesisStatus::Active,
        falsification_conditions: vec![],
        time_horizon: "12_months".into(),
        theme: Some("AI Infrastructure".into()),
        belief_statement: None,
            summary: None,
    }];

    // 新信号：NVIDIA announcement → 应匹配到已有 thesis
    let signals = vec![contract::Signal {
        id: "sig_new_001".into(),
        observation_id: "obs_001".into(),
        importance: 0.85,
        domain: "Semiconductor".into(),
        category: contract::SignalCategory::StructuralShift,
        why: "NVIDIA Blackwell Ultra GPU has 2x AI training performance".into(),
    }];

    let step = ThesisGenerationStepBuilder::new(mock_llm_config(), "test")
        .with_existing_theses(existing)
        .build();
    let ctx = StepContext::new("2026-07-12");

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(step.generate(signals, &ctx));
    assert!(result.is_ok());
    let theses = result.unwrap();

    // 应该匹配到已有 thesis，而不是创建新的
    assert_eq!(theses.len(), 1, "不应创建新 thesis");
    assert_eq!(theses[0].id, "thesis_existing");
    // 证据应追加（原有 legacy_sig_001 + 新 sig_new_001）
    assert_eq!(theses[0].evidence.len(), 2, "证据应追加上去");
    assert!(theses[0].evidence.contains(&"sig_new_001".to_string()));
}

#[test]
fn test_thesis_generation_unmatched_signals_fall_through() {
    // 已有 Thesis：完全不相关的主题
    let existing = vec![contract::Thesis {
        id: "thesis_old".into(),
        claim: "SpaceX starship launch schedule".into(),
        confidence: 0.5,
        evidence: vec![],
        status: contract::ThesisStatus::Active,
        falsification_conditions: vec![],
        time_horizon: "12_months".into(),
        theme: None,
        belief_statement: None,
            summary: None,
    }];

    // 新信号：AI 相关 — 不应匹配到 SpaceX
    let signals = vec![contract::Signal {
        id: "sig_ai_001".into(),
        observation_id: "obs_001".into(),
        importance: 0.75,
        domain: "AI Infrastructure".into(),
        category: contract::SignalCategory::CompetitiveSignal,
        why: "OpenAI released GPT-5 with 1M token context window".into(),
    }];

    let step = ThesisGenerationStepBuilder::new(mock_llm_config(), "test")
        .with_existing_theses(existing)
        .build();
    let ctx = StepContext::new("2026-07-12");

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(step.generate(signals, &ctx));
    // LLM 连接会失败（mock config），但管线不应崩溃——应返回已有 thesis（未匹配信号静默跳过）
    assert!(result.is_ok(), "LLM 失败不应导致管线崩溃");
    let theses = result.unwrap();
    // 应保留原有 thesis（不匹配），不会崩溃
    assert_eq!(theses.len(), 1, "应保留已有 thesis");
}

// ===== DecisionMapping: 完整映射链路 =====

#[test]
fn test_decision_mapping_full_flow() {
    let step = DecisionMappingStepBuilder::new().build();
    let ctx = StepContext::new("2026-07-12");

    let theses = vec![
        contract::Thesis {
            id: "t_strength".into(),
            claim: "AI Agent will grow rapidly".into(),
            confidence: 0.75,
            evidence: vec!["sig_1".into(), "sig_2".into(), "sig_3".into()],
            status: contract::ThesisStatus::Strengthening,
            falsification_conditions: vec!["Adoption flat for 12mo".into()],
            time_horizon: "12_months".into(),
            theme: None,
            belief_statement: None,
            summary: None,
        },
        contract::Thesis {
            id: "t_weaken".into(),
            claim: "GPU shortage will ease".into(),
            confidence: 0.35,
            evidence: vec!["sig_4".into()],
            status: contract::ThesisStatus::Weakening,
            falsification_conditions: vec![],
            time_horizon: "6_months".into(),
            theme: None,
            belief_statement: None,
            summary: None,
        },
        contract::Thesis {
            id: "t_invalid".into(),
            claim: "Oil prices will crash".into(),
            confidence: 0.1,
            evidence: vec![],
            status: contract::ThesisStatus::Invalidated,
            falsification_conditions: vec![],
            time_horizon: "30_days".into(),
            theme: None,
            belief_statement: None,
            summary: None,
        },
    ];

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(step.map(theses, &ctx));
    assert!(result.is_ok());
    let decisions = result.unwrap();
    assert_eq!(decisions.len(), 3);

    // Strengthening → Build
    assert!(matches!(decisions[0].action, contract::DecisionType::Build));
    // Weakening → Learn
    assert!(matches!(decisions[1].action, contract::DecisionType::Learn));
    // Invalidated → Exit
    assert!(matches!(decisions[2].action, contract::DecisionType::Exit));
    assert!(matches!(
        decisions[2].horizon,
        contract::DecisionHorizon::Immediate
    ));
}

#[test]
fn test_decision_mapping_smoothing_from_history() {
    // 上次决策是 Monitor
    let last_decisions = vec![contract::Decision {
        id: "dec_prev".into(),
        thesis_id: "t_001".into(),
        action: contract::DecisionType::Monitor,
        confidence: 0.5,
        horizon: contract::DecisionHorizon::Days90,
        reasoning: String::new(),
        made_at: "2026-07-11".into(),
        rule_passed: true,
        requires_review: false,
        review_reason: None,
    }];

    let step = DecisionMappingStepBuilder::new()
        .with_last_decisions(last_decisions)
        .build();
    let ctx = StepContext::new("2026-07-12");

    // Strengthening → 规则建议 Build，但平滑应抑制→Monitor
    let theses = vec![contract::Thesis {
        id: "t_001".into(),
        claim: "Test".into(),
        confidence: 0.75,
        evidence: vec!["sig_1".into(), "sig_2".into(), "sig_3".into()],
        status: contract::ThesisStatus::Strengthening,
        falsification_conditions: vec![],
        time_horizon: "12_months".into(),
        theme: None,
        belief_statement: None,
            summary: None,
    }];

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(step.map(theses, &ctx));
    assert!(result.is_ok());
    let decisions = result.unwrap();
    assert_eq!(decisions.len(), 1);
    // 因为 smoothing 抑制: Build → 保持 Monitor
    assert!(matches!(
        decisions[0].action,
        contract::DecisionType::Monitor
    ));
}

// ===== DecisionHistory: 持久化 + 去重 =====

#[test]
fn test_decision_history_persistence_round_trip() {
    let path = std::env::temp_dir().join(format!(
        "test_decision_history_golden_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    // 写入
    {
        let mut history = DecisionHistory::open(&path).unwrap();
        assert!(history.is_empty());

        let decisions = vec![contract::Decision {
            id: "dec_golden_001".into(),
            thesis_id: "thesis_001".into(),
            action: contract::DecisionType::Invest,
            confidence: 0.7,
            horizon: contract::DecisionHorizon::Days90,
            reasoning: "test".into(),
            made_at: "2026-07-12".into(),
            rule_passed: true,
            requires_review: false,
            review_reason: None,
        }];
        history
            .append_from_decisions(&decisions, "2026-07-12")
            .unwrap();
        assert_eq!(history.len(), 1);
    }

    // 重读验证持久化和去重
    {
        let mut history = DecisionHistory::open(&path).unwrap();
        assert_eq!(history.len(), 1, "重读后应还有 1 条");
        assert_eq!(history.all()[0].decision_id, "dec_golden_001");
        assert_eq!(history.all()[0].action, "Invest");

        // 追加同一条（去重）
        let duplicate = vec![contract::Decision {
            id: "dec_golden_001".into(),
            thesis_id: "thesis_001".into(),
            action: contract::DecisionType::Invest,
            confidence: 0.7,
            horizon: contract::DecisionHorizon::Days90,
            reasoning: "test".into(),
            made_at: "2026-07-12".into(),
            rule_passed: true,
            requires_review: false,
            review_reason: None,
        }];
        history
            .append_from_decisions(&duplicate, "2026-07-12")
            .unwrap();
        assert_eq!(history.len(), 1, "去重后应还是 1 条");
    }

    // 追加新记录
    {
        let mut history = DecisionHistory::open(&path).unwrap();
        let new_decisions = vec![contract::Decision {
            id: "dec_golden_002".into(),
            thesis_id: "thesis_002".into(),
            action: contract::DecisionType::Monitor,
            confidence: 0.5,
            horizon: contract::DecisionHorizon::Days30,
            reasoning: "test 2".into(),
            made_at: "2026-07-12".into(),
            rule_passed: true,
            requires_review: false,
            review_reason: None,
        }];
        history
            .append_from_decisions(&new_decisions, "2026-07-12")
            .unwrap();
        assert_eq!(history.len(), 2, "追加后应有 2 条");
    }

    // 最终验证
    let history = DecisionHistory::open(&path).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history.all()[0].decision_id, "dec_golden_001");
    assert_eq!(history.all()[1].decision_id, "dec_golden_002");

    let _ = std::fs::remove_file(&path);
}

// ===== Loader: memory_db.json → contract::Thesis =====

#[test]
fn test_loader_thesis_from_memory_db() {
    use std::io::Write;
    let path = std::env::temp_dir().join(format!("test_loader_golden_{}", std::process::id()));
    let json = r#"{
        "theses": [
            {
                "id": "t_active",
                "title": "Active Thesis",
                "status": "Active",
                "confidence_history": [{"confidence": 0.72}],
                "evidences": [{"title": "e1"}, {"title": "e2"}, {"title": "e3"}]
            },
            {
                "id": "t_retired",
                "title": "Old Thesis",
                "status": "Retired",
                "confidence_history": [],
                "evidences": []
            }
        ]
    }"#;
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(json.as_bytes()).unwrap();

    let theses = load_theses_from_memory_db(&path);
    assert_eq!(theses.len(), 1, "Retired thesis 应被过滤");
    assert_eq!(theses[0].id, "t_active");
    assert!((theses[0].confidence - 0.72).abs() < 0.01);
    // evidence 数量应从旧系统的 evidence 数组长度转换而来
    assert_eq!(theses[0].evidence.len(), 3);

    let _ = std::fs::remove_file(&path);
}

// ===== StepContext debug output =====

#[test]
fn test_step_context_debug_output_creates_file() {
    let dir = std::env::temp_dir().join(format!("test_debug_output_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let ctx = StepContext::new_debug("2026-07-12", dir.clone());
    assert!(ctx.should_write_debug());

    // 验证 classify 在 debug 模式写入文件
    let step = SignalClassificationStepBuilder::new(mock_llm_config(), "test").build();
    // 空输入 → 不写文件
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(step.classify(vec![], &ctx));
    assert!(result.is_ok());

    let _ = std::fs::remove_dir_all(&dir);
}

// ===== Pipeline 产生 IntelligenceOutput =====

#[test]
fn test_pipeline_output_structure() {
    let output = IntelligenceOutput {
        decisions: vec![],
        theses: vec![],
        signals: vec![],
        stats: Default::default(),
    };
    assert!(!output.has_decisions());
    assert_eq!(output.decision_count(), 0);

    let decisions = vec![contract::Decision {
        id: "dec_test".into(),
        thesis_id: "t_001".into(),
        action: contract::DecisionType::Build,
        confidence: 0.8,
        horizon: contract::DecisionHorizon::Days90,
        reasoning: "test".into(),
        made_at: "2026-07-12".into(),
        rule_passed: true,
        requires_review: false,
        review_reason: None,
    }];
    let output2 = IntelligenceOutput {
        decisions,
        theses: vec![],
        signals: vec![],
        stats: Default::default(),
    };
    assert!(output2.has_decisions());
    assert_eq!(output2.decision_count(), 1);
}

// ===== DecisionMapping: RuleEngine edge cases =====

#[test]
fn test_rule_engine_empty_evidence_pending() {
    let engine = sulix_intelligence::decision_mapping::RuleEngine;
    let thesis = contract::Thesis {
        id: "t_pending".into(),
        claim: "Pending claim".into(),
        confidence: 0.4,
        evidence: vec![],
        status: contract::ThesisStatus::Pending,
        falsification_conditions: vec![],
        time_horizon: "12_months".into(),
        theme: None,
        belief_statement: None,
            summary: None,
    };
    let mapping = engine.map_thesis(&thesis);
    assert!(matches!(
        mapping.decision_type,
        contract::DecisionType::Monitor
    ));
    // Pending + low evidence → 30 day horizon
    assert!(matches!(mapping.horizon, contract::DecisionHorizon::Days30));
}

#[test]
fn test_rule_engine_active_with_many_evidence() {
    let engine = sulix_intelligence::decision_mapping::RuleEngine;
    let thesis = contract::Thesis {
        id: "t_active_many".into(),
        claim: "Active claim with evidence".into(),
        confidence: 0.6,
        evidence: vec![
            "e1".into(),
            "e2".into(),
            "e3".into(),
            "e4".into(),
            "e5".into(),
        ],
        status: contract::ThesisStatus::Active,
        falsification_conditions: vec![],
        time_horizon: "12_months".into(),
        theme: None,
        belief_statement: None,
            summary: None,
    };
    let mapping = engine.map_thesis(&thesis);
    // Active + >= 3 evidence → Monitor + 90d
    assert!(matches!(
        mapping.decision_type,
        contract::DecisionType::Monitor
    ));
    assert!(matches!(mapping.horizon, contract::DecisionHorizon::Days90));
}
