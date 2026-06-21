//! 渲染模块 — 咨询级简报
//!
//! 抄 Reference/ 中 BCG/Deloitte/GS/McKinsey 报告结构
//! 抄 strategy-skills/ 中表格输出、Fact Base、Assumption Register、Kill List
//!
//! 核心格式：
//! 1. 执行摘要（BCG: Key Messages）
//! 2. 关键主题（McKinsey: 分类分层 + Fact Base 表格）
//! 3. 综合判断 + 假设审计（GS: 多源汇一结论 + Assumption Register）
//! 4. 战略建议 + 选项评估 + Kill List（Deloitte: How to Start）
//! 5. 数据源索引 + 认知校准

use anyhow::Result;
use chrono::Local;

use crate::clusterer::{Assumption, Theme, ThemeAnalysis, Summary};
use crate::fetcher::Article;

/// 渲染咨询级简报
pub fn render_daily_report(
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    summary: &Summary,
    calibration: Option<&str>,
    watchlist: Option<&[Article]>,
) -> Result<String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let mut md = String::new();

    // ── 标题 ──
    md.push_str(&format!("# Sulix Intelligence — {}\n\n", today));

    // ── 1. 执行摘要（抄 BCG: Key Messages）──
    md.push_str("## 执行摘要\n\n");
    if analyses.is_empty() {
        md.push_str("> 今日无聚类主题。所有信号均为孤立事件，不足以形成判断。\n\n");
    } else {
        for (i, a) in analyses.iter().enumerate() {
            md.push_str(&format!(
                "{}. **{}** — {}（{} 条证据）\n",
                i + 1,
                a.bluf,
                a.impact,
                a.fact_base.len(),
            ));
        }
        md.push('\n');
    }
    md.push_str("---\n\n");

    // ── 2. 关键主题（抄 McKinsey: 分类分层 + 抄 strategy-skills: Fact Base 表格）──
    for a in analyses {
        md.push_str(&format!("## 主题: {}\n\n", a.theme_title));

        // Fact Base 表格（抄 situation-assessment: Evidence | Interpretation | Confidence）
        if !a.fact_base.is_empty() {
            md.push_str("| 证据 | 解读 | 置信度 |\n");
            md.push_str("|------|------|--------|\n");
            for fb in &a.fact_base {
                md.push_str(&format!("| {} | {} | {} |\n", fb.evidence, fb.interpretation, fb.confidence));
            }
            md.push('\n');
        }

        // 综合影响 + 信号强度
        md.push_str(&format!("**信号强度**: {}/10 — ", a.signal_strength));
        if a.signal_strength >= 7 {
            md.push_str("行业机制级\n\n");
        } else if a.signal_strength >= 5 {
            md.push_str("竞争格局级\n\n");
        } else {
            md.push_str("单点事件级\n\n");
        }
        md.push_str(&format!("**影响**: {}\n\n", a.impact));

        // Phase 1: 蓝军 — 承重假设
        let load_bearing: Vec<&Assumption> = a.assumptions.iter().filter(|a| a.load_bearing).collect();
        if !load_bearing.is_empty() {
            md.push_str("**承重假设**:\n");
            for asm in &load_bearing {
                md.push_str(&format!("- {}（证据强度: {}）\n", asm.text, asm.evidence_strength));
            }
            md.push('\n');
        }

        // Phase 1: 蓝军 — 逆境情景
        if let Some(ref adverse) = a.adverse {
            if !adverse.scenario.is_empty() {
                md.push_str(&format!("**逆境情景**: {}。\n", adverse.scenario));
                md.push_str(&format!("**早期预警**: {}\n\n", adverse.early_warning));
            }
        }

        // Phase 1: 蓝军 — 待验证
        if !a.next_tests.is_empty() {
            md.push_str("**待验证**:\n");
            for test in &a.next_tests {
                md.push_str(&format!("- {}\n", test));
            }
            md.push('\n');
        }

        // 关联
        if !a.connections.is_empty() {
            md.push_str(&format!("**关联**: {}\n\n", a.connections.join(" → ")));
        }

        // 溯源
        if !a.source_urls.is_empty() {
            md.push_str("**溯源**:\n");
            for url in &a.source_urls {
                md.push_str(&format!("- {}\n", url));
            }
            md.push('\n');
        }

        md.push_str("---\n\n");
    }

    // ── 3. 综合判断（抄 GS: 结论先行，一句话定性 + BCG: Key Messages）──
    md.push_str("## 综合判断\n\n");
    if !analyses.is_empty() {
        // 结论先行（GS style: 一句话定性）
        if let Some(highest) = analyses.iter().max_by_key(|a| a.signal_strength) {
            md.push_str(&format!("**结论**: {}。\n\n", highest.bluf));
        } else if let Some(first) = analyses.first() {
            md.push_str(&format!("**结论**: {}。\n\n", first.bluf));
        }

        // 关键证据（列 top evidence）
        let key_evidence: Vec<String> = analyses.iter()
            .flat_map(|a| a.fact_base.iter())
            .filter(|fb| fb.confidence == "L1" || fb.confidence == "L2")
            .take(3)
            .map(|fb| format!("- {}（{}）", fb.interpretation, fb.confidence))
            .collect();
        if !key_evidence.is_empty() {
            md.push_str("**关键证据**:\n");
            for e in &key_evidence {
                md.push_str(e);
                md.push('\n');
            }
            md.push('\n');
        }

        // 风险提示（GS style: downside / caveat）
        md.push_str("**风险提示**: 上述判断依赖以下假设——\n");
        let risky: Vec<&str> = analyses.iter()
            .filter(|a| a.signal_strength < 5)
            .map(|_| "单一信号来源，需更多交叉验证")
            .collect();
        if risky.is_empty() {
            md.push_str("多源交叉验证充分，置信度较高。\n\n");
        } else {
            md.push_str("- 单一信号来源，需更多交叉验证\n");
            md.push('\n');
        }
    } else {
        md.push_str("**结论**: 今日无足够信号形成综合判断。\n\n");
    }
    md.push_str("---\n\n");

    // ── 4. 战略建议（抄 strategic-options: 选项评估 + Kill List）──
    md.push_str("## 战略建议\n\n");

    if !analyses.is_empty() {
        // 选项评估表
        md.push_str("### 选项评估\n\n");
        md.push_str("| 选项 | 逻辑 | 必须为真的前提 | 风险 | 信心 |\n");
        md.push_str("|------|------|--------------|------|------|\n");

        let has_commoditization = analyses.iter().any(|a|
            a.theme_title.contains("商品") || a.theme_title.contains("模型") || a.theme_title.contains("价格"));
        let has_reliability = analyses.iter().any(|a|
            a.theme_title.contains("可靠") || a.theme_title.contains("Agent"));
        let has_policy = analyses.iter().any(|a|
            a.theme_title.contains("政策") || a.theme_title.contains("风险") || a.theme_title.contains("芯片"));

        if has_commoditization || has_reliability {
            md.push_str("| 继续应用层深挖 | 模型商品化加速，窗口打开 | 价格战不压缩利润空间 | 模型厂商向下整合 | L3 |\n");
        }
        if has_policy {
            md.push_str("| 增加多模型适配 | 政策风险上升 | 多模型维护成本可控 | 复杂度拖慢迭代 | L3 |\n");
        }
        md.push_str("| 暂不调整 | 信号不足以改变路线 | 窗口期不会关闭 | 错过先发优势 | L4 |\n");
        md.push('\n');

        // Kill List（抄 initiative-prioritizer: 明确不做什么）
        md.push_str("### 不做什么（Kill List）\n\n");
        md.push_str("| 事项 | 原因 | 重回条件 |\n");
        md.push_str("|------|------|---------|\n");
        md.push_str("| Agent 框架对比研究 | 已商品化，差异化空间小 | Agent 协议层出现根本性变革 |\n");
        md.push_str("| 模型能力深度评测 | 决策价值递减 | 模型代际差距重新拉大 |\n");
        if !has_policy {
            md.push_str("| 多模型适配改造 | 当前无政策紧迫性 | 主要供应商受限或被限制 |\n");
        }
        md.push('\n');

        // 建议
        md.push_str("### 下一步\n\n");
        if has_commoditization {
            md.push_str("- **应用层深挖**: 本周选择一个垂直行业，做 3 个用户访谈\n");
        }
        if has_reliability {
            md.push_str("- **Agent 交付**: 用现有工具链跑通一个端到端场景\n");
        }
        if has_policy {
            md.push_str("- **政策追踪**: 关注 Fable 出口限制进展\n");
        }
        md.push_str("- **每周复盘**: 检查假设是否成立\n\n");
    } else {
        md.push_str("今日不建议做任何策略调整。继续执行当前计划。\n\n");
    }

    md.push_str("---\n\n");

    // ── 5. 数据源索引 ──
    md.push_str("## 数据源索引\n\n");
    md.push_str("| 信号 | 来源 |\n|------|------|\n");
    for a in analyses {
        if let Some(t) = themes.iter().find(|t| t.id == a.theme_id) {
            for art in &t.articles {
                md.push_str(&format!("| {} | {} |\n", art.title, art.source));
            }
        }
    }
    md.push('\n');

    // ── 🟡 Watchlist（v1.1 弱信号观察层）──
    if let Some(watch_articles) = watchlist {
        if !watch_articles.is_empty() {
            md.push_str("## 🟡 正在跟踪（Watchlist）\n\n");
            md.push_str("以下信号不足以进入关键主题，但保留观察，多源交叉后将升级：\n\n");
            for art in watch_articles {
                md.push_str(&format!("- **{}** — {}\n", art.title, art.source));
            }
            md.push('\n');
            md.push_str("---\n\n");
        }
    }

    // ── 认知校准 ──
    if let Some(calibration) = calibration {
        if !calibration.is_empty() {
            md.push_str("────────────────────────────────────────\n\n");
            md.push_str(&format!("🤖 **认知校准**\n\n> {}\n\n", calibration));
            md.push_str("（不回答也没事，看到就行）\n\n");
            md.push_str("────────────────────────────────────────\n\n");
        }
    }

    // ── 脚注（抄 strategy-skills: Quality Bar）──
    md.push_str("---\n\n");
    md.push_str(&format!(
        "*本期简报覆盖 {} 个主题，{} 条证据。生成于 {}.*\n",
        summary.theme_count,
        summary.total_articles,
        Local::now().format("%Y-%m-%d %H:%M"),
    ));
    md.push_str("*审计链: data/2026-06-21/*\n");
    md.push_str("*质量标准: 决策导向 | 假设显性 | 证据感知 | 可操作*\n");

    Ok(md)
}
