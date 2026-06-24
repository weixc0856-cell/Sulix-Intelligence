use crate::db::TrendRow;
use crate::engine::memory::{Thesis, ThesisStatus, Stance, Outcome, Reflection, OutcomeType};
use crate::domain::evidence::Evidence;
use super::helpers::html_escape;

/// 渲染 Trend Layer 趋势区块
pub fn render_trend_block(trends: &[TrendRow]) -> String {
    if trends.is_empty() {
        return String::new();
    }
    let rows: String = trends
        .iter()
        .map(|t| {
            let (arrow, color) = if t.change_pct > 20.0 {
                ("↑", "#16a34a")
            } else if t.change_pct < -20.0 {
                ("↓", "#dc2626")
            } else {
                ("→", "#a3a3a3")
            };
            format!(
                r#"<div style="display:flex;justify-content:space-between;font-size:0.8125rem;padding:0.25rem 0;border-bottom:1px solid #f0f0f0">
  <span style="font-family:'Inter',sans-serif;color:#171717">{}</span>
  <span style="font-family:'JetBrains Mono',monospace;font-weight:600;color:{}">{}{:.0}% <span style="font-weight:400;color:#a3a3a3">({}→{})</span></span>
</div>"#,
                html_escape(&t.category),
                color,
                arrow,
                t.change_pct.abs(),
                t.prev_count,
                t.recent_count,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<div style="margin-top:1.5rem;padding:0.75rem;background:#fafafa;border-radius:0.25rem">
  <div style="font-family:'JetBrains Mono',monospace;font-size:0.75rem;font-weight:700;text-transform:uppercase;letter-spacing:0.05em;color:#171717;margin-bottom:0.5rem">📊 过去 14 天趋势 <span style="font-weight:400;color:#a3a3a3;font-size:0.625rem">近 7 天 vs 前 7 天</span></div>
  {}</div>"#,
        rows
    )
}

/// 渲染单个 Thesis 的支持/挑战证据时间线 HTML
fn render_evidence_timeline(evidences: &[Evidence]) -> String {
    if evidences.is_empty() {
        return String::from("<div style=\"font-size:0.75rem;color:#a3a3a3;padding:0.25rem 0\">暂无证据</div>");
    }

    // 按日期聚合统计
    let mut daily: Vec<(&str, usize, usize)> = Vec::new();
    for ev in evidences {
        let date = ev.date.as_str();
        if let Some(last) = daily.last_mut() {
            if last.0 == date {
                match ev.stance {
                    Stance::Supports => last.1 += 1,
                    Stance::Challenges => last.2 += 1,
                    Stance::Neutral => {}
                }
                continue;
            }
        }
        let (s, c) = match ev.stance {
            Stance::Supports => (1, 0),
            Stance::Challenges => (0, 1),
            Stance::Neutral => (0, 0),
        };
        daily.push((date, s, c));
    }

    let bars: String = daily.iter().rev().take(14).map(|(date, support, challenge)| {
        let total = (*support + *challenge).max(1);
        let s_pct = *support as f64 / total as f64 * 100.0;
        let c_pct = *challenge as f64 / total as f64 * 100.0;
        format!(
            r#"<div style="display:flex;align-items:center;gap:0.375rem;margin-bottom:0.125rem">
  <span style="font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#a3a3a3;min-width:4.5rem">{}</span>
  <div style="flex:1;height:0.75rem;background:#f0f0f0;border-radius:0.125rem;display:flex;overflow:hidden">
    <div style="width:{}%;background:#16a34a;height:100%"></div>
    <div style="width:{}%;background:#dc2626;height:100%"></div>
  </div>
  <span style="font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#525252;min-width:2.5rem;text-align:right">↑{} ↓{}</span>
</div>"#,
            date,
            s_pct, c_pct,
            support, challenge,
        )
    }).collect::<Vec<_>>().join("\n");

    format!(
        r#"<div style="margin:0.5rem 0;padding:0.5rem;background:#fafafa;border-radius:0.25rem">
  <div style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;font-weight:600;color:#525252;text-transform:uppercase;margin-bottom:0.375rem">📈 证据时间线 (近 14 天)</div>
  {}</div>"#,
        bars,
    )
}

/// 渲染 Outcome/Reflection 区块
fn render_outcome_section(outcomes: &[Outcome], reflections: &[Reflection]) -> String {
    if outcomes.is_empty() && reflections.is_empty() {
        return String::new();
    }

    let outcome_rows: String = outcomes.iter().map(|o| {
        let icon = match o.result {
            OutcomeType::Confirmed => "✅",
            OutcomeType::PartiallyConfirmed => "🟡",
            OutcomeType::Refuted => "❌",
            OutcomeType::Inconclusive => "❓",
        };
        format!(
            r#"<div style="display:flex;align-items:flex-start;gap:0.375rem;padding:0.375rem 0;border-bottom:1px solid #f0f0f0;font-size:0.75rem">
  <span>{}</span>
  <div><span style="color:#171717;font-weight:500">{}</span>
    <div style="color:#737373;font-size:0.6875rem;margin-top:0.125rem">预期: {} → 实际: {}</div>
  </div>
</div>"#,
            icon,
            o.recorded_at,
            html_escape(&o.expected),
            html_escape(&o.actual),
        )
    }).collect::<Vec<_>>().join("\n");

    let reflection_rows: String = reflections.iter().map(|r| {
        format!(
            r#"<div style="padding:0.375rem 0;border-bottom:1px solid #f0f0f0;font-size:0.75rem">
  <div style="color:#525252;font-weight:500">📝 {} </div>
  <div style="color:#ef4444;font-size:0.6875rem;margin-top:0.125rem">原因: {}</div>
  <div style="color:#737373;font-size:0.6875rem">{}</div>
</div>"#,
            r.created_at,
            html_escape(&r.error_reason),
            r.lessons.join(" · "),
        )
    }).collect::<Vec<_>>().join("\n");

    let mut html = String::new();
    if !outcome_rows.is_empty() {
        html.push_str(&format!(r#"<div style="margin:0.5rem 0;padding:0.5rem;background:#fafafa;border-radius:0.25rem">
  <div style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;font-weight:600;color:#525252;text-transform:uppercase;margin-bottom:0.375rem">🎯 判断结果追踪 (Outcome)</div>
  {}</div>"#, outcome_rows));
    }
    if !reflection_rows.is_empty() {
        html.push_str(&format!(r#"<div style="margin:0.5rem 0;padding:0.5rem;background:#fef2f2;border-radius:0.25rem">
  <div style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;font-weight:600;color:#ef4444;text-transform:uppercase;margin-bottom:0.375rem">🔍 反思复盘 (Reflection)</div>
  {}</div>"#, reflection_rows));
    }
    html
}

/// 渲染 Memory Engine Thesis 看板（增强版）
///
/// 展示：
/// - 所有 Thesis 状态树 + Evidence 时间线
/// - Outcome/Reflection 数据
/// - 支持/挑战证据条形图
/// 输出到 `<vault>/memory/index.html`
pub fn render_memory_dashboard(
    theses: &[Thesis],
    outcomes: &[Outcome],
    reflections: &[Reflection],
) -> String {
    let items: String = theses
        .iter()
        .map(|t| {
            let status_icon = match t.status {
                ThesisStatus::Active => "🟢",
                ThesisStatus::Strengthening => "🔵",
                ThesisStatus::Weakening => "🟡",
                ThesisStatus::Retired => "⚪",
            };
            let status_text = format!("{:?}", t.status);
            let evidence_count = t.evidences.len();
            let support_count = t.evidences.iter().filter(|e| e.stance == Stance::Supports).count();
            let challenge_count = t.evidences.iter().filter(|e| e.stance == Stance::Challenges).count();
            let confidence = if evidence_count > 0 {
                (support_count as f64 / evidence_count as f64 * 100.0) as u8
            } else {
                0
            };

            // 关联的 Outcome/Reflection
            let thesis_outcomes: Vec<Outcome> = outcomes.iter().filter(|o| o.thesis_id == t.id).cloned().collect();
            let thesis_reflections: Vec<Reflection> = reflections.iter().filter(|r| r.thesis_id == t.id).cloned().collect();

            let ev_rows: String = t
                .evidences
                .iter()
                .rev()
                .take(10)
                .map(|e| {
                    let stance_icon = match e.stance {
                        Stance::Supports => "↑",
                        Stance::Challenges => "↓",
                        Stance::Neutral => "→",
                    };
                    format!(
                        r#"<tr>
                <td style="font-size:0.6875rem;color:#737373">{}</td>
                <td style="font-size:0.6875rem;font-weight:600;color:{}">{}</td>
                <td style="font-size:0.6875rem;color:#171717">{}</td>
                <td style="font-size:0.6875rem;color:#525252">{}</td>
              </tr>"#,
                        html_escape(&e.date),
                        if e.stance == Stance::Supports { "#16a34a" } else if e.stance == Stance::Challenges { "#dc2626" } else { "#a3a3a3" },
                        stance_icon,
                        html_escape(&e.title),
                        html_escape(&e.summary),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            let timeline = render_evidence_timeline(&t.evidences);
            let outcome_section = render_outcome_section(&thesis_outcomes, &thesis_reflections);

            format!(
                r#"<div style="border-bottom:1px solid #e5e5e5;padding:1rem 0">
  <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.25rem">
    <span style="font-size:1rem">{}</span>
    <span style="font-family:'Inter',sans-serif;font-weight:600;color:#171717;font-size:0.9375rem">{}</span>
    <span style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;background:#f5f5f5;padding:0.125rem 0.375rem;border-radius:0.125rem">{}</span>
  </div>
  <div style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;margin-bottom:0.375rem">
    创建 {} · 更新 {} · 共 {} 条证据 (↑{} ↓{}) · 置信度 {}%
  </div>
  {}
  {}
  <details style="margin-top:0.375rem">
    <summary style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;cursor:pointer">📋 证据列表 (近 10 条)</summary>
    <table style="width:100%;border-collapse:collapse;margin-top:0.375rem">{}</table>
  </details>
</div>"#,
                status_icon,
                html_escape(&t.title),
                status_text,
                t.created,
                t.updated,
                evidence_count,
                support_count,
                challenge_count,
                confidence,
                timeline,
                outcome_section,
                ev_rows,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let total_active = theses.iter().filter(|t| t.status != ThesisStatus::Retired).count();
    let total_outcomes = outcomes.len();
    let total_reflections = reflections.len();

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
  <title>Thesis Dashboard | Sulix Memory</title>
  <style>body{{font-family:'Inter',-apple-system,sans-serif;background:#fcfcfc;color:#111;margin:0}}a{{color:#e3120b;text-decoration:none}}details>summary{{list-style:none}}details>summary::before{{content:"▶ ";font-size:0.625rem}}details[open]>summary::before{{content:"▼ ";font-size:0.625rem}}</style>
</head>
<body>
<div style="max-width:56rem;margin:0 auto;padding:1.5rem 1rem">
  <div style="border-bottom:2px solid #171717;padding-bottom:0.75rem;margin-bottom:1rem;display:flex;align-items:baseline;justify-content:space-between">
    <h1 style="font-family:'JetBrains Mono',monospace;font-size:1.25rem;font-weight:700;margin:0">🧠 Thesis Dashboard</h1>
    <div style="text-align:right;font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3">
      <div>{} theses ({} active) · {} outcomes · {} reflections</div>
    </div>
  </div>
  {}
</div>
</body>
</html>"#,
        theses.len(),
        total_active,
        total_outcomes,
        total_reflections,
        items,
    )
}
