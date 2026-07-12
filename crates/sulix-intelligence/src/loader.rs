//! Loader — 从旧系统加载已有状态到新管线
//!
//! 在新管线与旧系统并行运行期间，需要从旧系统加载已存在的状态
//! （如 MemoryEngine 中已有的 Thesis），以保证 ThesisGenerationStep
//! 能在已有判断基础上追加证据，而不是每次都重新生成。

use std::io::Write;
use std::path::Path;

use sulix_contract as contract;

/// 从 memory_db.json 加载已有 Thesis 列表
///
/// 此函数读取旧 MemoryEngine 的持久化文件，提取 Thesis 数据，
/// 转换为新管线使用的 `contract::Thesis` 类型。
///
/// # 格式兼容
/// - 旧系统的 `domain::thesis::Thesis.title` → `contract::Thesis.claim`
/// - 旧系统的 `domain::thesis::Thesis.status` → `contract::ThesisStatus`
/// - Evidence 数量 → `evidence` 列表
///
/// # 错误处理
/// - 文件不存在 → 返回空 Vec（非错误）
/// - 解析失败 → 返回空 Vec + log warning
pub fn load_theses_from_memory_db(path: &Path) -> Vec<contract::Thesis> {
    if !path.exists() {
        return vec![];
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("⚠️ 无法读取 memory_db.json ({}), 返回空 thesis 列表", e);
            return vec![];
        }
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("⚠️ 解析 memory_db.json 失败 ({}), 返回空 thesis 列表", e);
            return vec![];
        }
    };

    let theses_array = match json["theses"].as_array() {
        Some(arr) => arr,
        None => return vec![],
    };

    let theses: Vec<contract::Thesis> = theses_array
        .iter()
        .filter_map(|entry| {
            let id = entry["id"].as_str().unwrap_or("").to_string();
            let title = entry["title"].as_str().unwrap_or("").to_string();
            let confidence = entry["confidence_history"]
                .as_array()
                .and_then(|h| h.last())
                .and_then(|p| {
                    // contract::Thesis uses "value"; legacy data uses "confidence"
                    p["value"].as_f64().or_else(|| p["confidence"].as_f64())
                })
                .unwrap_or(0.5);

            if id.is_empty() {
                return None;
            }

            // Map status
            let status = match entry["status"].as_str() {
                Some("Proposed") => contract::ThesisStatus::Proposed,
                Some("Active") => contract::ThesisStatus::Active,
                Some("Strengthening") => contract::ThesisStatus::Strengthening,
                Some("Weakening") => contract::ThesisStatus::Weakening,
                Some("Dormant") | Some("Retired") => return None, // 跳过非活跃 thesis
                _ => contract::ThesisStatus::Active,
            };

            // Extract evidence count (used as signal ID placeholders)
            let evidence_count = entry["evidences"].as_array().map(|e| e.len()).unwrap_or(0);

            // Read falsification_conditions (new pipeline field, may be absent in legacy data)
            let falsifications: Vec<String> = entry["falsification_conditions"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            Some(contract::Thesis {
                id,
                claim: title,
                confidence,
                evidence: (0..evidence_count)
                    .map(|i| format!("legacy_{}", i))
                    .collect(),
                status,
                falsification_conditions: falsifications,
                time_horizon: "12_months".into(),
                theme: entry["primary_domain"].as_str().map(String::from),
                belief_statement: None,
                summary: None,
            })
        })
        .collect();

    log::info!(
        "📂 load_theses: 从 memory_db 加载 {} 条已有 Thesis",
        theses.len()
    );
    theses
}

/// 将新管线的 Thesis 输出写回 MemoryEngine 的 memory_db.json
///
/// # 合并策略
/// - 匹配到已有 thesis（按 id）→ 更新 updated 时间戳 + falsification_conditions
/// - 未匹配 → 作为新 thesis 追加
/// - 不删除旧 thesis（安全的 append-only 合并）
///
/// # 错误处理
/// - 文件不存在 → 创建新文件（首次运行）
/// - 解析失败 → log warning，跳过写入
pub fn save_theses_to_memory_db(path: &Path, new_theses: &[contract::Thesis]) {
    if new_theses.is_empty() {
        return;
    }

    // 读取或初始化 JSON
    let mut root: serde_json::Value = if path.exists() {
        match std::fs::read_to_string(path) {
            Ok(c) => serde_json::from_str(&c).unwrap_or_else(|e| {
                log::warn!("⚠️ 解析 memory_db.json 失败 ({}), 重新创建", e);
                serde_json::json!({"theses": []})
            }),
            Err(e) => {
                log::warn!("⚠️ 读取 memory_db.json 失败 ({}), 重新创建", e);
                serde_json::json!({"theses": []})
            }
        }
    } else {
        serde_json::json!({"theses": []})
    };

    let Some(theses_array) = root.get_mut("theses").and_then(|v| v.as_array_mut()) else {
        log::warn!("⚠️ memory_db.json 格式异常: 'theses' 不是数组");
        return;
    };

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut new_count = 0usize;
    let mut update_count = 0usize;

    for new_thesis in new_theses {
        // 尝试按 id 匹配已有 thesis
        let existing = theses_array
            .iter_mut()
            .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(&new_thesis.id));

        match existing {
            Some(entry) => {
                // 更新已有 thesis
                entry["updated"] = serde_json::json!(today);
                if !new_thesis.falsification_conditions.is_empty() {
                    entry["falsification_conditions"] =
                        serde_json::to_value(&new_thesis.falsification_conditions)
                            .unwrap_or_default();
                }
                update_count += 1;
            }
            None => {
                // 创建新 thesis（映射 contract::Thesis → domain::thesis::Thesis JSON）
                let status_str = match new_thesis.status {
                    contract::ThesisStatus::Proposed => "Proposed",
                    contract::ThesisStatus::Active => "Active",
                    contract::ThesisStatus::Strengthening => "Strengthening",
                    contract::ThesisStatus::Weakening => "Weakening",
                    contract::ThesisStatus::Pending => "Active",
                    contract::ThesisStatus::Confirmed => "Active",
                    contract::ThesisStatus::Invalidated => "Retired",
                    contract::ThesisStatus::Dormant => "Dormant",
                    contract::ThesisStatus::Retired => "Retired",
                };

                let new_entry = serde_json::json!({
                    "id": new_thesis.id,
                    "title": new_thesis.claim,
                    "created": today,
                    "updated": today,
                    "evidences": [],
                    "assumptions": [],
                    "status": status_str,
                    "confidence_history": [{
                        "date": today,
                        "value": new_thesis.confidence,
                        "trigger": "Initial",
                        "reason": "由 IntelligencePipeline 创建"
                    }],
                    "status_history": [],
                    "falsification_conditions": new_thesis.falsification_conditions,
                    "decision_history": [],
                    "primary_domain": new_thesis.theme.clone().unwrap_or_default(),
                    "lifecycle_events": [{
                        "date": today,
                        "kind": {"kind": "Created", "detail": {}}
                    }]
                });

                theses_array.push(new_entry);
                new_count += 1;
            }
        }
    }

    // 写回文件
    if new_count > 0 || update_count > 0 {
        let json = serde_json::to_string_pretty(&root).unwrap_or_default();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::File::create(path) {
            Ok(mut file) => {
                file.write_all(json.as_bytes()).unwrap_or_else(|e| {
                    log::warn!("⚠️ memory_db.json 写入失败: {}", e);
                });
            }
            Err(e) => {
                log::warn!("⚠️ memory_db.json 创建失败: {}", e);
            }
        }
        log::info!(
            "📂 save_theses: {} 新增, {} 更新 → memory_db.json",
            new_count,
            update_count
        );
    }
}

/// 从 DecisionHistory 加载上次决策
///
/// 将 DecisionHistory 中的记录转换为 contract::Decision 列表。
/// 失败时返回空 Vec，调用方忽略即可。
pub fn load_last_decisions(path: &Path) -> Vec<contract::Decision> {
    match crate::decision_history::DecisionHistory::open(path) {
        Ok(history) => {
            let count = history.len();
            log::info!("  📜 DecisionHistory: {} 条历史", count);
            history
                .all()
                .iter()
                .map(|r| {
                    let action = match r.action.as_str() {
                        "Build" => contract::DecisionType::Build,
                        "Invest" => contract::DecisionType::Invest,
                        "Monitor" => contract::DecisionType::Monitor,
                        "Learn" => contract::DecisionType::Learn,
                        "Ignore" => contract::DecisionType::Ignore,
                        "Exit" => contract::DecisionType::Exit,
                        _ => contract::DecisionType::Monitor,
                    };
                    contract::Decision {
                        id: r.decision_id.clone(),
                        thesis_id: r.thesis_id.clone(),
                        action,
                        confidence: r.confidence,
                        horizon: contract::DecisionHorizon::Days90,
                        reasoning: String::new(),
                        made_at: r.made_at.clone(),
                        rule_passed: true,
                        requires_review: false,
                        review_reason: None,
                    }
                })
                .collect()
        }
        Err(_) => {
            log::info!("  📜 DecisionHistory: 新实例");
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_theses_from_empty_file() {
        let path = std::env::temp_dir().join("test_empty_memory.json");
        let result = load_theses_from_memory_db(&path);
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_theses_from_valid_json() {
        let path = std::env::temp_dir().join("test_valid_memory.json");
        let json = r#"{
            "theses": [
                {
                    "id": "t1",
                    "title": "AI Agent adoption will grow",
                    "status": "Active",
                    "confidence_history": [{"confidence": 0.65}],
                    "evidences": [{"title": "ev1"}, {"title": "ev2"}]
                },
                {
                    "id": "t2",
                    "title": "GPU supply will tighten",
                    "status": "Strengthening",
                    "confidence_history": [{"confidence": 0.7}],
                    "evidences": []
                }
            ]
        }"#;
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(json.as_bytes()).unwrap();

        let theses = load_theses_from_memory_db(&path);
        assert_eq!(theses.len(), 2);
        assert_eq!(theses[0].claim, "AI Agent adoption will grow");
        assert!((theses[0].confidence - 0.65).abs() < 0.01);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_theses_creates_new_file() {
        let path = std::env::temp_dir().join(format!("test_save_new_{}", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let theses = vec![contract::Thesis {
            id: "thesis_new_001".into(),
            claim: "New pipeline thesis".into(),
            confidence: 0.65,
            evidence: vec![],
            status: contract::ThesisStatus::Active,
            falsification_conditions: vec!["Adoption flat for 12mo".into()],
            time_horizon: "12_months".into(),
            theme: Some("AI Infrastructure".into()),
            belief_statement: None,
                summary: None,
        }];

        save_theses_to_memory_db(&path, &theses);
        assert!(path.exists(), "memory_db.json 应被创建");

        // 读回验证
        let loaded = load_theses_from_memory_db(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].claim, "New pipeline thesis");
        assert!((loaded[0].confidence - 0.65).abs() < 0.01);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_theses_updates_existing() {
        let path = std::env::temp_dir().join(format!("test_save_update_{}", std::process::id()));
        let _ = std::fs::remove_file(&path);

        // 先存一条
        save_theses_to_memory_db(
            &path,
            &[contract::Thesis {
                id: "t1".into(),
                claim: "Original".into(),
                confidence: 0.5,
                evidence: vec![],
                status: contract::ThesisStatus::Active,
                falsification_conditions: vec![],
                time_horizon: "12_months".into(),
                theme: None,
                belief_statement: None,
                summary: None,
            }],
        );

        // 更新同一条 + 新增一条
        save_theses_to_memory_db(
            &path,
            &[
                contract::Thesis {
                    id: "t1".into(),
                    claim: "Original".into(),
                    confidence: 0.7,
                    evidence: vec!["sig_new".into()],
                    status: contract::ThesisStatus::Strengthening,
                    falsification_conditions: vec!["New condition".into()],
                    time_horizon: "12_months".into(),
                    theme: None,
                    belief_statement: None,
                summary: None,
                },
                contract::Thesis {
                    id: "t2".into(),
                    claim: "New thesis".into(),
                    confidence: 0.6,
                    evidence: vec![],
                    status: contract::ThesisStatus::Proposed,
                    falsification_conditions: vec![],
                    time_horizon: "6_months".into(),
                    theme: None,
                    belief_statement: None,
                summary: None,
                },
            ],
        );

        // 读回验证：应有 2 条，t1 的 falsification_conditions 已更新
        let loaded = load_theses_from_memory_db(&path);
        assert_eq!(loaded.len(), 2, "应包含原有 + 新增");
        let t1 = loaded.iter().find(|t| t.id == "t1").unwrap();
        assert_eq!(
            t1.falsification_conditions.len(),
            1,
            "t1 的 falsification_conditions 应被更新"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_theses_empty_noop() {
        let path = std::env::temp_dir().join(format!("test_save_noop_{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        save_theses_to_memory_db(&path, &[]);
        assert!(!path.exists(), "空输入不应创建文件");
    }
}
