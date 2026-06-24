use std::collections::BTreeSet;

use anyhow::Result;

use crate::clusterer::{Theme, ThemeAnalysis, ChangeSummary};
use crate::config::SourceConfig;

use super::helpers::{html_escape, svi_color, svi_emoji};
use super::seo::{render_seo_meta, render_json_ld};

// ===== Terminal Dashboard (Bloomberg Terminal 风格) =====

/// 渲染 Bloomberg Terminal 风格的 HTML 简报
#[allow(clippy::too_many_arguments)]
pub fn render_html_report(
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    date: &str,
    calibration: Option<&str>,
    attributable_sources: &[SourceConfig],
    flash_headline: Option<&str>,
    language: &str,
    source_statuses: &[(String, bool, usize)],
    change_summary: Option<&ChangeSummary>,
) -> Result<String> {
    let attributable_names = crate::source::attributable_source_names(attributable_sources);

    // 按 SVI 降序排列
    let mut indexed: Vec<(&Theme, &ThemeAnalysis)> = themes
        .iter()
        .zip(analyses.iter())
        .filter(|(_, a)| a.signal_strength > 0)
        .collect();
    use std::cmp::Reverse;
    indexed.sort_by_key(|(_, a)| Reverse(a.signal_strength));

    let mut signals_html = String::new();
    let mut explicit_set: BTreeSet<String> = BTreeSet::new();
    let mut implicit_set: BTreeSet<String> = BTreeSet::new();

    for (theme, analysis) in &indexed {
        let svi = analysis.signal_strength;
        let is_premium = svi >= 7;

        let mut srcs: Vec<String> = Vec::new();
        for art in &theme.articles {
            if !srcs.contains(&art.source) {
                srcs.push(art.source.clone());
            }
        }
        for s in &srcs {
            if attributable_names.contains(s) {
                explicit_set.insert(s.clone());
            } else {
                implicit_set.insert(s.clone());
            }
        }

        let summary = if analysis.bluf.len() > 80 {
            let end = analysis.bluf.floor_char_boundary(80);
            format!("{}...", &analysis.bluf[..end])
        } else {
            analysis.bluf.clone()
        };

        let prem = if is_premium {
            let slug = theme.title.to_lowercase().replace(' ', "-");
            format!(
                r#"<a href="../premium/{}.html" style="color:#ea580c;font-family:'JetBrains Mono',monospace;font-size:0.65rem;font-weight:600;text-transform:uppercase;letter-spacing:0.05em;text-decoration:none;border:1px solid #ea580c;padding:0.0625rem 0.375rem;border-radius:0.125rem">🔒 Premium</a>"#,
                html_escape(&slug)
            )
        } else {
            String::new()
        };

        let line = format!(
            r#"<div style="display:flex;flex-direction:column;padding:0.5rem 0;border-bottom:1px solid #e5e5e5">
  <div style="display:flex;align-items:center;gap:0.5rem">
    <span style="color:{};font-family:'JetBrains Mono',monospace;font-weight:700;font-size:0.8125rem">{}</span>
    <span style="color:{};font-family:'JetBrains Mono',monospace;font-weight:700;font-size:0.8rem;min-width:3ch">{:.1}</span>
    <span style="font-family:'Inter',sans-serif;font-size:0.875rem;font-weight:500;color:#171717;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{}</span>
    <span style="margin-left:auto">{}</span>
  </div>
  <div style="font-family:'Inter',sans-serif;font-size:0.75rem;color:#525252;margin-top:0.125rem;padding-left:4.5rem">{}</div>
  <div style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;margin-top:0.125rem;padding-left:4.5rem">├─ 来源: {}</div>
</div>"#,
            svi_color(svi),
            svi_emoji(svi),
            svi_color(svi),
            svi as f64,
            html_escape(&theme.title),
            prem,
            html_escape(&summary),
            html_escape(&srcs.join(" · "))
        );
        signals_html.push_str(&line);
    }

    let explicit_links: Vec<String> = explicit_set.iter().map(|s| format!(r#"<span style="font-size:0.75rem;font-family:'JetBrains Mono',monospace;color:#525252">{} </span>"#, html_escape(s))).collect();
    let implicit_links: Vec<String> = implicit_set.iter().map(|s| format!(r#"<span style="font-size:0.75rem;font-family:'JetBrains Mono',monospace;color:#a3a3a3">{} </span>"#, html_escape(s))).collect();

    let cal_html = match calibration {
        Some(cal) if !cal.is_empty() => format!(
            r#"<div style="border-top:1px solid #e5e5e5;margin-top:1.5rem;padding-top:0.75rem"><span style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;font-weight:600;text-transform:uppercase">Cognitive Calibration</span><p style="font-family:'Inter',sans-serif;font-size:0.8125rem;color:#737373;font-style:italic;margin:0.25rem 0 0">{}</p></div>"#,
            html_escape(cal)
        ),
        _ => String::new(),
    };

    let flash = flash_headline.map(|fh| format!(r#"<div style="background-color:#dc2626;color:#fff;text-align:center;padding:0.375rem;font-family:'JetBrains Mono',monospace;font-size:0.75rem;font-weight:600">⚡ FLASH: {}</div>"#, html_escape(fh))).unwrap_or_default();

    let top = analyses.iter().max_by_key(|a| a.signal_strength);
    let seo_title = top
        .map(|a| a.theme_title.as_str())
        .unwrap_or("Daily Briefing");
    let seo_desc = top.map(|a| a.bluf.as_str()).unwrap_or("Sulix Intelligence");
    let lang_attr = if language == "zh" { "zh-Hant" } else { "en" };
    let seo_meta = render_seo_meta(seo_title, seo_desc, &format!("en/{}/", date));
    let json_ld = render_json_ld(seo_title, date, seo_desc);
    let d = if date.len() >= 10 {
        format!("{}-{}-{}", &date[0..4], &date[5..7], &date[8..10])
    } else {
        date.to_string()
    };
    let is_zh = language == "zh";
    let en_href = if is_zh {
        format!("../en/{}/index.html", date)
    } else {
        "#".into()
    };
    let zh_href = if !is_zh {
        format!("../zh/{}/index.html", date)
    } else {
        "#".into()
    };
    let en_s = if !is_zh {
        "color:#171717;font-weight:700"
    } else {
        "color:#a3a3a3"
    };
    let zh_s = if is_zh {
        "color:#171717;font-weight:700"
    } else {
        "color:#a3a3a3"
    };

    // Change Summary 区块
    let change_html = match change_summary {
        Some(cs) => {
            let mut h = String::from(
                r#"<div style="border-left:3px solid #2563eb;padding:0.5rem 0.75rem;margin-bottom:0.75rem;background:#fafafa;font-family:'JetBrains Mono',monospace;font-size:0.75rem">"#,
            );
            if cs.conflicts.is_empty() && cs.new_signals.is_empty() {
                h.push_str(&format!(
                    r#"<span style="color:#16a34a">✓ 无异动 — {} 条信号强化既有判断</span>"#,
                    cs.no_change_count
                ));
            } else {
                if !cs.conflicts.is_empty() {
                    h.push_str(&format!(r#"<div style="color:#dc2626;margin-bottom:0.25rem">⚡ {} 条信号与既有判断冲突</div>"#, cs.conflicts.len()));
                    for c in &cs.conflicts {
                        h.push_str(&format!(r#"<div style="padding-left:0.75rem;font-size:0.6875rem;color:#525252">✗ <strong>{}</strong>: {}</div>"#, html_escape(&c.topic), html_escape(&c.today_signal)));
                    }
                }
                if !cs.new_signals.is_empty() {
                    h.push_str(&format!(
                        r#"<div style="color:#2563eb;margin-top:0.25rem">★ {} 条新信号: {}</div>"#,
                        cs.new_signals.len(),
                        cs.new_signals
                            .iter()
                            .map(|s| html_escape(s))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                if cs.no_change_count > 0 {
                    h.push_str(&format!(r#"<div style="color:#a3a3a3;margin-top:0.25rem">○ {} 条不改变当前判断</div>"#, cs.no_change_count));
                }
            }
            h.push_str("</div>");
            h
        }
        None => String::new(),
    };

    // Source Health 区块
    let source_health_html = if source_statuses.is_empty() {
        String::new()
    } else {
        let (mut healthy, mut degraded, mut dead): (Vec<&str>, Vec<&str>, Vec<&str>) =
            (vec![], vec![], vec![]);
        for (name, ok, count) in source_statuses {
            if !ok || *count == 0 {
                dead.push(name);
            } else if *count <= 2 {
                degraded.push(name);
            } else {
                healthy.push(name);
            }
        }
        let mut html = String::from(
            r#"<div style="margin-top:0.75rem;padding-top:0.375rem;border-top:1px solid #e5e5e5;font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#a3a3a3">▸ SOURCE HEALTH "#,
        );
        if !dead.is_empty() {
            html.push_str(&format!(
                r#"<span style="color:#dc2626">✗ {}无数据</span> "#,
                dead.join("·")
            ));
        }
        if !degraded.is_empty() {
            html.push_str(&format!(
                r#"<span style="color:#ca8a04">△ {}低流量</span> "#,
                degraded.join("·")
            ));
        }
        html.push_str(&format!("✓ {}源正常", healthy.len()));
        html.push_str("</div>");
        html
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="{}">
<head>
<meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>Sulix.Intel | {}</title>
<link rel="stylesheet" href="./design.css">
<link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><rect width='100' height='100' fill='%23e3120b'/><text y='75' x='35' font-family='sans-serif' font-weight='900' font-size='70' fill='white'>i</text></svg>">
{}{}
<style>body{{font-family:'Inter',sans-serif;background:#fcfcfc;color:#111;margin:0}}a{{text-decoration:none}}a:hover{{text-decoration:underline}}</style>
</head>
<body>
{}
<header style="border-bottom:1px solid #e5e5e5;background:#fff"><div style="max-width:72rem;margin:0 auto;padding:0 1rem;height:3rem;display:flex;align-items:center;justify-content:space-between">
<a href="/" style="display:flex;align-items:center;gap:0.5rem"><span style="background-color:#e3120b;color:#fff;font-family:'JetBrains Mono',monospace;font-weight:700;font-size:0.75rem;padding:0.125rem 0.375rem;border-radius:0.125rem">i</span><span style="font-family:'JetBrains Mono',monospace;font-weight:700;font-size:0.9375rem;color:#171717">Sulix.Intel</span></a>
<nav style="display:flex;align-items:center;gap:0.75rem">
<a href="{}" style="font-family:'JetBrains Mono',monospace;font-size:0.6875rem;{}">EN</a><span style="color:#d4d4d4;font-size:0.6875rem">|</span>
<a href="{}" style="font-family:'JetBrains Mono',monospace;font-size:0.6875rem;{}">繁中</a>
<a href="https://sulix.substack.com/subscribe" target="_blank" style="background-color:#e3120b;color:#fff;font-family:'JetBrains Mono',monospace;font-size:0.625rem;font-weight:600;padding:0.25rem 0.5rem;border-radius:0.125rem;text-transform:uppercase;letter-spacing:0.05em">Subscribe →</a>
</nav></div></header>
<main style="max-width:72rem;margin:0 auto;padding:1rem 1rem 2rem">
<div style="display:flex;align-items:baseline;justify-content:space-between;margin-bottom:0.75rem;padding-bottom:0.5rem;border-bottom:2px solid #171717">
<h1 style="font-family:'JetBrains Mono',monospace;font-size:1.25rem;font-weight:700;color:#171717;margin:0">今日信号</h1>
<span style="font-family:'JetBrains Mono',monospace;font-size:0.6875rem;color:#a3a3a3">{} · {} 条</span>
</div>
{}
<div>{}</div>
{}
{}
</main>
<footer style="max-width:72rem;margin:1.5rem auto 2rem;padding:0 1rem"><div style="border-top:1px solid #e5e5e5;padding-top:1rem">
<div style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;font-weight:600;text-transform:uppercase;margin-bottom:0.5rem">📚 Primary Sources & Traces</div>
<div style="font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#525252;font-weight:600;text-transform:uppercase;letter-spacing:0.08em;margin-bottom:0.25rem">═══ Explicit Citation ═══</div>
<div style="display:flex;flex-wrap:wrap;gap:0.375rem;margin-bottom:0.5rem">{}</div>
<div style="font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#a3a3a3;font-weight:600;text-transform:uppercase;letter-spacing:0.08em;margin-bottom:0.25rem">═══ Implicit Intelligence ═══</div>
<div style="display:flex;flex-wrap:wrap;gap:0.375rem;margin-bottom:0.75rem">{}</div>
<p style="font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#a3a3a3;line-height:1.5;margin:0">* Sulix operates under Fair Use. Data from publicly available primary documents.</p>
</div>
{}

<div style="margin-top:1rem;padding-top:0.5rem;border-top:1px solid #e5e5e5;font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#a3a3a3">Sulix.Intel · intel.getsulix.com · Substack · GitHub · MIT · Generated {}</div>
</footer>
</body>
</html>"#,
        lang_attr, html_escape(seo_title), seo_meta, json_ld,
        flash,
        en_href, en_s, zh_href, zh_s,
        d, indexed.len(),
        change_html,
        signals_html,
        flash_headline.map(|_| format!(r#"<div style="margin-top:0.5rem;padding-top:0.5rem;border-top:1px solid #e5e5e5;display:flex;gap:1rem;font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3"><span>{} 条信号</span><span style="color:#dc2626">🔴 Flash</span></div>"#, indexed.len())).unwrap_or_default(),
        cal_html,
        if explicit_links.is_empty() { "<span style=\"font-size:0.75rem;font-family:'JetBrains Mono',monospace;color:#a3a3a3\">No explicit citations</span>".into() } else { explicit_links.join("") },
        if implicit_links.is_empty() { "<span style=\"font-size:0.75rem;font-family:'JetBrains Mono',monospace;color:#a3a3a3\">No implicit sources</span>".into() } else { implicit_links.join("") },
        source_health_html,
        chrono::Local::now().format("%Y-%m-%d %H:%M UTC"),
    );

    Ok(html)
}

/// 渲染编年史看板总页面（Economist Graphic Detail 版式）
pub fn render_archive_dashboard(entries: &[crate::archive::ChronicleEntry]) -> Result<String> {
    let list_html: String = entries.iter().map(|item| {
        let entities_badges: String = item.entities.iter()
            .map(|e| format!("<span class='text-[10px] font-mono bg-neutral-100 text-neutral-600 px-1.5 py-0.5 rounded-sm'>{}</span>", html_escape(e)))
            .collect::<Vec<_>>().join(" ");

        format!(
            r#"<div class="group border-b border-neutral-100 py-4 flex flex-col md:flex-row md:items-baseline md:justify-between hover:bg-neutral-50/50 px-2 transition-colors">
                <div class="flex items-baseline gap-4">
                  <span class="text-xs font-mono text-neutral-400 font-semibold w-24 shrink-0">{}</span>
                  <div class="space-y-1">
                    <span class="text-xs font-bold text-[#e3120b] uppercase tracking-wider block text-[10px]">{}</span>
                    <span class="chronicle-title text-lg font-bold text-neutral-900 group-hover:text-[#e3120b] transition-colors">{}</span>
                  </div>
                </div>
                <div class="mt-2 md:mt-0 flex gap-1.5">{}</div>
              </div>"#,
            item.date,
            html_escape(&item.topic),
            html_escape(&item.headline),
            entities_badges
        )
    }).collect::<Vec<_>>().join("\n");

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Geopolitical Tech Chronicle | Sulix</title>
  <style>body{{font-family:'Inter',-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background-color:#fcfcfc;color:#111;}}.chronicle-title{{font-family:'Lora','Playfair Display','Georgia',serif;}}</style>
</head>
<body>
  <div class="h-[4px] w-full bg-[#e3120b]"></div>
  <header class="border-b border-neutral-100 bg-white">
    <div class="max-w-5xl mx-auto px-4 h-14 flex items-center justify-between sm:px-6 lg:px-8">
      <a href="/" class="flex items-center gap-2.5 no-underline group select-none">
        <div class="w-6 h-6 bg-[#e3120b] flex items-center justify-center rounded-xs shadow-[0_1px_2px_rgba(0,0,0,0.05)]">
          <span class="text-white font-sans font-black text-sm tracking-tighter leading-none relative -top-[0.5px]" style="font-family: Inter">i</span>
        </div>
        <div class="flex items-baseline tracking-tight">
          <span class="text-lg font-bold text-neutral-900" style="font-family: 'Lora', 'Playfair Display', 'Georgia', serif;">Sulix</span>
          <span class="text-lg font-light text-neutral-300 mx-0.5">.</span>
          <span class="text-xs font-semibold tracking-widest text-neutral-400 uppercase" style="font-family: Inter;">Intel</span>
        </div>
      </a>
      <nav class="flex items-center gap-3 text-[11px] font-semibold tracking-wider text-neutral-400" style="font-family: Inter">
        <button onclick="toggleLang('en')" id="l-en" class="font-bold border-b-2 border-neutral-900 pb-0.5 cursor-pointer">EN</button>
        <span class="text-neutral-300">|</span>
        <button onclick="toggleLang('zh')" id="l-zh" class="text-neutral-400 hover:text-neutral-900 cursor-pointer">繁中</button>
      </nav>
    </div>
  </header>

  <div class="max-w-4xl mx-auto px-4 py-8">
    <div class="border-b-2 border-neutral-950 pb-6">
      <h1 class="chronicle-title text-4xl sm:text-5xl font-bold tracking-tight text-neutral-900">Geopolitical Tech Chronicle</h1>
      <p class="chronicle-title text-lg italic text-neutral-500 mt-2">A long-arc systemic tracker tracing geopolitical frictions down to technology supply lines.</p>
      <div class="mt-3 text-xs text-neutral-400">{} entries spanning {} topics</div>
    </div>
    <div class="mt-8 space-y-1">
      <div class="text-xs font-bold uppercase tracking-wider text-neutral-400 border-b border-neutral-200 pb-2 px-2">Historical Event Feed</div>
      {}
    </div>
  </div>
<script>
function toggleLang(t){{var p=window.location.pathname;if(p.endsWith('index.html')){{p=p.substring(0,p.lastIndexOf('index.html'))}}
if(t==='zh'){{if(!p.startsWith('/zh/')){{var ce=p.startsWith('/en/')?p.substring(3):p;window.location.pathname='/zh'+(ce.startsWith('/')?ce:'/'+ce)}}}}
else if(t==='en'){{if(p.startsWith('/zh/')){{var cz=p.substring(3);window.location.pathname=(cz==='/'||cz==='')?'/':'/en'+cz}}else if(p==='/'||p===''){{window.location.pathname='/en/'}}}}}}
(function(){{var pp=window.location.pathname,zh=pp.startsWith('/zh/');var el=document.getElementById('l-zh');var ee=document.getElementById('l-en');if(el&&ee){{el.className=zh?'font-bold border-b-2 border-neutral-900 pb-0.5 text-neutral-900':'text-neutral-400 hover:text-neutral-900 cursor-pointer';ee.className=zh?'text-neutral-400 hover:text-neutral-900 cursor-pointer':'font-bold border-b-2 border-neutral-900 pb-0.5 text-neutral-900'}}}}}})()
</script>
</body>
</html>"#,
        entries.len(),
        entries
            .iter()
            .map(|e| e.topic.as_str())
            .collect::<std::collections::HashSet<&str>>()
            .len(),
        list_html,
    );

    Ok(html)
}
