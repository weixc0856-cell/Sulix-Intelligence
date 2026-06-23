//! 设计令牌系统 — 所有样式值集中在此，renderer.rs 引用令牌名而非魔法值
//!
//! 抄 Wayne's blog tailwind.config.mjs 的设计令牌体系
//! generate_css() 输出单一 CSS 文件，解除对 Tailwind CDN 的依赖
#![allow(dead_code)]

// ===== 原子令牌：颜色 =====
pub const RED: &str = "#e3120b";
pub const CHARCOAL: &str = "#111111";
pub const NEUTRAL_300: &str = "#d4d4d4";
pub const NEUTRAL_400: &str = "#a3a3a3";
pub const NEUTRAL_500: &str = "#737373";
pub const NEUTRAL_900: &str = "#171717";
pub const BG: &str = "#fcfcfc";
pub const WHITE: &str = "#ffffff";

// ===== 原子令牌：字体 =====
pub const FONT_BODY: &str =
    "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif";
pub const FONT_SERIF: &str = "'Lora', 'Playfair Display', 'Georgia', serif";
pub const FONT_MONO: &str = "'JetBrains Mono', 'Courier New', monospace";

// ===== 复合令牌：排版（font-size / line-height / letter-spacing / weight）=====
// 参考 Wayne's blog: display-xl / headline-lg / body-md / label-sm 体系
pub const CSS_DISPLAY: &str = "font-size:1.875rem;line-height:1.25;font-weight:700;letter-spacing:-0.02em;font-family:'Lora','Georgia',serif;";
pub const CSS_HEADLINE: &str =
    "font-size:1.25rem;line-height:1.3;font-weight:700;font-family:'Lora','Georgia',serif;";
pub const CSS_BODY: &str = "font-size:0.9375rem;line-height:1.6;font-family:'Inter',sans-serif;";
pub const CSS_LABEL: &str = "font-size:0.75rem;line-height:1.2;font-weight:600;letter-spacing:0.05em;text-transform:uppercase;font-family:'Inter',sans-serif;";
pub const CSS_META: &str =
    "font-size:0.625rem;line-height:1.2;font-family:'JetBrains Mono',monospace;";

// ===== 间距韵律（8px 基准）=====
pub const SPACING_XS: &str = "4px";
pub const SPACING_SM: &str = "8px";
pub const SPACING_MD: &str = "24px";
pub const SPACING_LG: &str = "48px";

// ===== 生成 design.css =====
pub fn generate_css() -> String {
    format!(
        r#"/* Sulix Intelligence Design System */
/* 自动生成，请勿手工编辑 */
/* 抄 Wayne's blog: tailwind.config.mjs 设计令牌体系 */

@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@500;600&family=Lora:ital,wght@0,700;1,400&display=swap');

:root {{
  --red: {RED};
  --charcoal: {CHARCOAL};
  --neutral-300: {NEUTRAL_300};
  --neutral-400: {NEUTRAL_400};
  --neutral-500: {NEUTRAL_500};
  --neutral-900: {NEUTRAL_900};
  --bg: {BG};
  --white: {WHITE};
  --font-body: {FONT_BODY};
  --font-serif: {FONT_SERIF};
  --font-mono: {FONT_MONO};
}}

/* 基础重置 */
body {{
  font-family: var(--font-body);
  background-color: var(--bg);
  color: var(--charcoal);
  -webkit-font-smoothing: antialiased;
}}

/* 排版语义类 — 对齐 Wayne's blog 纯文字美学 */
.intel-display {{
  font-family: var(--font-serif);
  font-size: 1.875rem;
  line-height: 1.25;
  font-weight: 700;
  letter-spacing: -0.02em;
}}
@media (min-width: 640px) {{
  .intel-display {{ font-size: 2.25rem; }}
}}

.intel-headline {{
  font-family: var(--font-serif);
  font-size: 1.25rem;
  font-weight: 700;
  line-height: 1.3;
}}

.intel-body {{
  font-family: var(--font-body);
  font-size: 0.9375rem;
  line-height: 1.6;
}}

.intel-label {{
  font-family: var(--font-body);
  font-size: 0.75rem;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}}

.intel-meta {{
  font-family: var(--font-mono);
  font-size: 0.625rem;
  line-height: 1.2;
}}

/* 组件类 */
.intel-red-bar {{
  height: 4px;
  width: 100%;
  background-color: var(--red);
}}

.intel-tag {{
  display: inline-block;
  font-family: var(--font-mono);
  font-size: 0.625rem;
  background-color: #f5f5f5;
  color: #525252;
  padding: 0.125rem 0.375rem;
  border-radius: 0.125rem;
  border: 1px solid #e5e5e5;
}}

.intel-entity-badge {{
  font-family: var(--font-mono);
  font-size: 0.625rem;
  background-color: #f5f5f5;
  color: #525252;
  padding: 0.125rem 0.5rem;
  border-radius: 0.125rem;
  border: 1px solid #e5e5e5;
}}

/* 卡片软阴影 — 抄 Wayne's blog soft-card */
.intel-card {{
  background-color: var(--white);
  border-radius: 0.5rem;
  border: 1px solid #e5e5e580;
  box-shadow: 0 2px 8px rgba(0,0,0,0.02);
}}

.intel-card-header {{
  font-family: var(--font-mono);
  font-size: 0.625rem;
  font-weight: 600;
  letter-spacing: 0.05em;
}}

/* 《经济学人》红标签 */
.intel-category-tag {{
  color: var(--red);
  font-size: 0.625rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}}
"#,
        RED = RED,
        CHARCOAL = CHARCOAL,
        NEUTRAL_300 = NEUTRAL_300,
        NEUTRAL_400 = NEUTRAL_400,
        NEUTRAL_500 = NEUTRAL_500,
        NEUTRAL_900 = NEUTRAL_900,
        BG = BG,
        WHITE = WHITE,
        FONT_BODY = FONT_BODY,
        FONT_SERIF = FONT_SERIF,
        FONT_MONO = FONT_MONO,
    )
}

// ===== Tailwind 兼容类（替换 CDN 运行时）=====

#[allow(dead_code)]
pub const LAYOUT_CONTAINER: &str = "\
.max-w-5xl { max-width: 64rem; } \
.max-w-4xl { max-width: 56rem; } \
.max-w-3xl { max-width: 48rem; } \
.mx-auto { margin-left: auto; margin-right: auto; } \
.px-4 { padding-left: 1rem; padding-right: 1rem; } \
.px-6 { padding-left: 1.5rem; padding-right: 1.5rem; } \
.py-12 { padding-top: 3rem; padding-bottom: 3rem; } \
.pt-8 { padding-top: 2rem; } \
.pb-12 { padding-bottom: 3rem; } \
";

#[allow(dead_code)]
pub const LAYOUT_GRID: &str = "\
.grid { display: grid; } \
.grid-cols-1 { grid-template-columns: repeat(1, minmax(0, 1fr)); } \
@media (min-width: 768px) { .md\\:grid-cols-3 { grid-template-columns: repeat(3, minmax(0, 1fr)); } } \
@media (min-width: 1024px) { .lg\\:grid-cols-3 { grid-template-columns: repeat(3, minmax(0, 1fr)); } } \
.gap-4 { gap: 1rem; } \
.gap-8 { gap: 2rem; } \
.lg\\:col-span-2 { grid-column: span 2 / span 2; } \
";

#[allow(dead_code)]
pub const BORDERS: &str = "\
.border { border-width: 1px; } \
.border-b { border-bottom-width: 1px; } \
.border-t { border-top-width: 1px; } \
.border-b-2 { border-bottom-width: 2px; } \
.border-neutral-100 { border-color: #f5f5f5; } \
.border-neutral-200 { border-color: #e5e5e5; } \
.border-neutral-950 { border-color: #0a0a0a; } \
.rounded-lg { border-radius: 0.5rem; } \
.rounded-sm { border-radius: 0.125rem; } \
";

#[allow(dead_code)]
pub const FLEX: &str = "\
.flex { display: flex; } \
.items-center { align-items: center; } \
.justify-between { justify-content: space-between; } \
.flex-wrap { flex-wrap: wrap; } \
.gap-1\\.5 { gap: 0.375rem; } \
.gap-2 { gap: 0.5rem; } \
.gap-3 { gap: 0.75rem; } \
";

#[allow(dead_code)]
pub const SPACING: &str = "\
.mt-2 { margin-top: 0.5rem; } \
.mt-3 { margin-top: 0.75rem; } \
.mt-6 { margin-top: 1.5rem; } \
.mt-8 { margin-top: 2rem; } \
.mb-2 { margin-bottom: 0.5rem; } \
.mb-3 { margin-bottom: 0.75rem; } \
.mb-4 { margin-bottom: 1rem; } \
.mb-6 { margin-bottom: 1.5rem; } \
.mb-8 { margin-bottom: 2rem; } \
.p-4 { padding: 1rem; } \
.p-5 { padding: 1.25rem; } \
.p-6 { padding: 1.5rem; } \
.pt-4 { padding-top: 1rem; } \
.pb-6 { padding-bottom: 1.5rem; } \
";

#[allow(dead_code)]
pub const TEXT_UTILITIES: &str = "\
.text-center { text-align: center; } \
.text-xs { font-size: 0.75rem; line-height: 1rem; } \
.text-sm { font-size: 0.875rem; line-height: 1.25rem; } \
.text-lg { font-size: 1.125rem; line-height: 1.75rem; } \
.text-3xl { font-size: 1.875rem; line-height: 2.25rem; } \
.font-bold { font-weight: 700; } \
.font-semibold { font-weight: 600; } \
.font-normal { font-weight: 400; } \
.font-light { font-weight: 300; } \
.uppercase { text-transform: uppercase; } \
.italic { font-style: italic; } \
.tracking-tight { letter-spacing: -0.025em; } \
.tracking-wider { letter-spacing: 0.05em; } \
.tracking-widest { letter-spacing: 0.1em; } \
.leading-tight { line-height: 1.25; } \
.leading-relaxed { line-height: 1.625; } \
";

#[allow(dead_code)]
pub const COLORS: &str = "\
.text-neutral-300 { color: #d4d4d4; } \
.text-neutral-400 { color: #a3a3a3; } \
.text-neutral-500 { color: #737373; } \
.text-neutral-700 { color: #404040; } \
.text-neutral-800 { color: #262626; } \
.text-neutral-900 { color: #171717; } \
.text-white { color: #ffffff; } \
.text-sky-800 { color: #075985; } \
.text-amber-600 { color: #d97706; } \
.text-amber-700 { color: #b45309; } \
.bg-white { background-color: #ffffff; } \
.bg-neutral-50 { background-color: #fafafa; } \
.bg-neutral-100 { background-color: #f5f5f5; } \
.bg-slate-100 { background-color: #f1f5f9; } \
.hover\\:text-red-600:hover { color: #dc2626; } \
.transition-colors { transition-property: color, background-color; transition-duration: 150ms; } \
";

#[allow(dead_code)]
pub const RED_BAR_CSS: &str = "\
.h-\\[4px\\] { height: 4px; } \
.w-full { width: 100%; } \
.bg-\\[\\#e3120b\\] { background-color: #e3120b; } \
";

/// 生成完整的 design.css（覆盖全部渲染器使用的样式）
/// 在 base generate_css() 基础上追加 Tailwind 兼容类
pub fn generate_full_css() -> String {
    let base = generate_css();
    format!(
        "{base}\n\n\
/* === Tailwind 兼容类（替换 CDN 运行时）=== */\n\n\
/* 布局容器 */\n{LAYOUT_CONTAINER}\n\n\
/* 网格 */\n{LAYOUT_GRID}\n\n\
/* 边框 */\n{BORDERS}\n\n\
/* 弹性布局 */\n{FLEX}\n\n\
/* 间距 */\n{SPACING}\n\n\
/* 文字工具 */\n{TEXT_UTILITIES}\n\n\
/* 颜色 */\n{COLORS}\n\n\
/* 红色顶部栏 */\n{RED_BAR}\n\n\
/* 响应式断点 */\n\
@media (min-width: 640px) {{ \
.sm\\:text-4xl {{ font-size: 2.25rem; line-height: 2.5rem; }} \
.sm\\:px-6 {{ padding-left: 1.5rem; padding-right: 1.5rem; }} \
}} \
@media (min-width: 1024px) {{ \
.lg\\:px-8 {{ padding-left: 2rem; padding-right: 2rem; }} \
.lg\\:col-span-2 {{ grid-column: span 2 / span 2; }} \
}}",
        base = base,
        LAYOUT_CONTAINER = LAYOUT_CONTAINER,
        LAYOUT_GRID = LAYOUT_GRID,
        BORDERS = BORDERS,
        FLEX = FLEX,
        SPACING = SPACING,
        TEXT_UTILITIES = TEXT_UTILITIES,
        COLORS = COLORS,
        RED_BAR = RED_BAR_CSS,
    )
}
