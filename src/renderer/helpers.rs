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

/// 转义 YAML 字符串中的特殊字符
///
/// 保护措施:
/// - 包含 `:`, `#`, `"`, `'` 时自动加引号并转义内部引号
/// - YAML 关键字 (`true`, `false`, `null`, `yes`, `no`, `on`, `off`) 自动加引号防类型强制
/// - 纯数字字符串自动加引号防类型强制
pub(crate) fn yaml_escape(s: &str) -> String {
    let needs_quoting = s.contains(':')
        || s.contains('#')
        || s.contains('"')
        || s.contains('\'')
        || s.contains('[')
        || s.contains(']')
        || matches!(s, "true" | "false" | "null" | "yes" | "no" | "on" | "off")
        || s.chars().all(|c| c.is_numeric());
    if needs_quoting {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}
