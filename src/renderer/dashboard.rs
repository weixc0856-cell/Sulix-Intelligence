use crate::db::TrendRow;
use crate::engine::memory::{Thesis, ThesisStatus, Stance};
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

/// 渲染 Memory Engine Thesis 看板
///
/// 展示所有 Thesis 的状态树 + Evidence 列表。
/// 输出到 `<vault>/memory/index.html`，通过 `intel.getsulix.com/memory/` 访问。
pub fn render_memory_dashboard(theses: &[Thesis]) -> String {
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
            let ev_rows: String = t
                .evidences
                .iter()
                .map(|e| {
                    let stance_icon = match e.stance {
                        Stance::Supports => "↑",
                        Stance::Challenges => "↓",
                        Stance::Neutral => "→",
                    };
                    format!(
                        r#"<tr>
                <td style="font-size:0.75rem;color:#737373">{}</td>
                <td style="font-size:0.75rem;color:#171717">{}</td>
                <td style="font-size:0.75rem;font-weight:600;color:{}">{}</td>
                <td style="font-size:0.75rem;color:#525252">{}</td>
              </tr>"#,
                        html_escape(&e.date),
                        html_escape(&e.title),
                        if e.stance == Stance::Supports {
                            "#16a34a"
                        } else if e.stance == Stance::Challenges {
                            "#dc2626"
                        } else {
                            "#a3a3a3"
                        },
                        stance_icon,
                        html_escape(&e.summary),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            format!(
                r#"<div style="border-bottom:1px solid #e5e5e5;padding:0.75rem 0">
        <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.25rem">
          <span style="font-size:1rem">{}</span>
          <span style="font-family:'Inter',sans-serif;font-weight:600;color:#171717;font-size:0.9375rem">{}</span>
          <span style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;background:#f5f5f5;padding:0.125rem 0.375rem;border-radius:0.125rem">{}</span>
        </div>
        <div style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;margin-bottom:0.5rem">创建 {} · 更新 {} · {} 条证据</div>
        <table style="width:100%;border-collapse:collapse">{}</table>
      </div>"#,
                status_icon,
                html_escape(&t.title),
                status_text,
                t.created,
                t.updated,
                evidence_count,
                ev_rows,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
  <title>Thesis Dashboard | Sulix Memory</title>
  <style>body{{font-family:'Inter',-apple-system,sans-serif;background:#fcfcfc;color:#111;margin:0}}a{{color:#e3120b;text-decoration:none}}</style>
</head>
<body>
<div style="max-width:56rem;margin:0 auto;padding:1.5rem 1rem">
  <div style="border-bottom:2px solid #171717;padding-bottom:0.75rem;margin-bottom:1rem;display:flex;align-items:baseline;justify-content:space-between">
    <h1 style="font-family:'JetBrains Mono',monospace;font-size:1.25rem;font-weight:700;margin:0">🧠 Thesis Dashboard</h1>
    <span style="font-family:'JetBrains Mono',monospace;font-size:0.6875rem;color:#a3a3a3">{} theses</span>
  </div>
  {}
</div>
</body>
</html>"#,
        theses.len(),
        items,
    )
}
