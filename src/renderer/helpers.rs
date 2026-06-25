/// HTML 实体转义。顺序严格：& 必须最先转义，防止双重编码。
pub(crate) fn html_escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// SVI 颜色色值
/// TODO: 当前未被调用，保留为 MDX/前端未来配色参考
#[allow(dead_code)]
pub(crate) fn svi_color(svi: u8) -> &'static str {
    match svi {
        9..=10 => "#dc2626",
        7..=8 => "#ea580c",
        5..=6 => "#ca8a04",
        3..=4 => "#16a34a",
        _ => "#2563eb",
    }
}

/// SVI 颜色表情
#[allow(dead_code)]
pub(crate) fn svi_emoji(svi: u8) -> &'static str {
    match svi {
        9..=10 => "🔴",
        7..=8 => "🟠",
        5..=6 => "🟡",
        3..=4 => "🟢",
        _ => "🔵",
    }
}
