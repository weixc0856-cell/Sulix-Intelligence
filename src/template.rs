//! 模板引擎 — 纯文本占位符替换（抄 Daily-News-Briefing template-engine.ts）
//!
//! 使用 Rust 原生 .replace()，零运行时开销。
//! 不支持逻辑分支/循环——{{TOPICS}} 由调用方预先渲染好传入。

use std::collections::HashMap;

/// 模板数据（分析报告 + 聚合共用）
pub struct TemplateData {
    // 元数据
    pub date: String,
    pub time: String,
    pub topic_count: usize,
    pub article_count: usize,
    pub processing_time: String,

    // 内容区（调用方已渲染好的块）
    pub executive_summary: String,
    pub topic_sections: String,  // 所有主题块拼接
    pub synthesis: String,
    pub decision_required: String,
    pub watchlist: String,
    pub calibration: String,
    pub source_index: String,
    pub processing_status: String,

    // YAML frontmatter 扩展字段
    pub metrics: HashMap<String, String>,
}

/// 渲染模板：替换所有 {{PLACEHOLDER}} 占位符
pub fn render(template: &str, data: &TemplateData) -> String {
    let mut result = template.to_string();

    // YAML frontmatter
    result = result.replace("{{YAML_FRONTMATTER}}", &build_frontmatter(data));
    // 元数据
    result = result.replace("{{DATE}}", &data.date);
    result = result.replace("{{TIME}}", &data.time);
    result = result.replace("{{TOPIC_COUNT}}", &data.topic_count.to_string());
    result = result.replace("{{ARTICLE_COUNT}}", &data.article_count.to_string());
    result = result.replace("{{PROCESSING_TIME}}", &data.processing_time);
    // 内容区
    result = result.replace("{{EXECUTIVE_SUMMARY}}", &data.executive_summary);
    result = result.replace("{{TOPICS}}", &data.topic_sections);
    result = result.replace("{{SYNTHESIS}}", &data.synthesis);
    result = result.replace("{{DECISION_REQUIRED}}", &data.decision_required);
    result = result.replace("{{WATCHLIST}}", &data.watchlist);
    result = result.replace("{{CALIBRATION}}", &data.calibration);
    result = result.replace("{{SOURCE_INDEX}}", &data.source_index);
    result = result.replace("{{PROCESSING_STATUS}}", &data.processing_status);

    result
}

/// 构建 YAML frontmatter（含 metrics 供后续量化分析工具回测）
fn build_frontmatter(data: &TemplateData) -> String {
    let mut fm = String::from("---\n");
    fm.push_str(&format!("date: \"{}\"\n", data.date));
    fm.push_str(&format!("generated_at: \"{}\"\n", data.time));
    fm.push_str(&format!("topics: [{}]\n", data.topic_count));
    fm.push_str(&format!("articles: {}\n", data.article_count));
    // metrics 字典（含资金流入、风险评分等硬指标，用于对账和回测）
    if !data.metrics.is_empty() {
        fm.push_str("metrics:\n");
        let mut keys: Vec<&String> = data.metrics.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(val) = data.metrics.get(key) {
                fm.push_str(&format!("  {}: {}\n", key, val));
            }
        }
    }
    fm.push_str("---\n\n");
    fm
}

// ===== 预设模板 =====

/// 分析报告模板（咨询级，含完整结构）
pub const ANALYSIS_TEMPLATE: &str = "\
{{YAML_FRONTMATTER}}\
# Sulix Intelligence — {{DATE}}

## 执行摘要

{{EXECUTIVE_SUMMARY}}
---

{{TOPICS}}

{{SYNTHESIS}}

{{DECISION_REQUIRED}}
---

## 数据源索引

{{SOURCE_INDEX}}

{{WATCHLIST}}

{{PROCESSING_STATUS}}

{{CALIBRATION}}
---

*本期简报覆盖 {{TOPIC_COUNT}} 个主题，{{ARTICLE_COUNT}} 条证据。生成于 {{TIME}}.*
*质量标准: 决策导向 | 假设显性 | 证据感知 | 可操作*
";

/// 信号聚合模板（简洁格式）
pub const AGGREGATION_TEMPLATE: &str = "\
{{YAML_FRONTMATTER}}\
*生成时间 {{TIME}}*

---

## Table of Contents

{{TOPICS}}

{{WATCHLIST}}

*生成时间 {{TIME}}*
";
