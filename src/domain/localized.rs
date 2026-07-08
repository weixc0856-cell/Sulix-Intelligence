//! Localized 多语言字段类型
//!
//! 三语言（en/zh-cn/zh-tw）容器，带确定性回退链。
//! 消费方必须通过 `get()` 访问，不得直接读取 .zh_cn / .zh_tw。
//! 构造方（翻译 agent）和 helper 自身豁免。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 三语言多文本字段
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Localized {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub en: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zh_cn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zh_tw: Option<String>,
}

impl Localized {
    /// 构造一个只有英文的 Localized（方便迁移期使用）
    pub fn en_only(text: &str) -> Self {
        Self {
            en: Some(text.to_string()),
            zh_cn: None,
            zh_tw: None,
        }
    }

    /// 判断是否所有语言均为空
    pub fn is_empty(&self) -> bool {
        self.en.as_deref().unwrap_or("").is_empty()
            && self.zh_cn.as_deref().unwrap_or("").is_empty()
            && self.zh_tw.as_deref().unwrap_or("").is_empty()
    }
}

/// 解析后的文本 + 解析元数据
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedText<'a> {
    pub text: &'a str,
    pub resolved_locale: &'a str,
    pub is_fallback: bool,
}

impl Localized {
    /// locale 归一化：zh-CN / zh_cn → zh-cn；zh-TW / zh_tw → zh-tw
    fn normalize_locale(locale: &str) -> &str {
        // We return a static mapping — only 3 known locales need normalization
        match locale {
            "zh-CN" | "zh_cn" => "zh-cn",
            "zh-TW" | "zh_tw" => "zh-tw",
            other => other,
        }
    }

    /// 取指定 locale 的值，空字符串视同 None
    fn get_locale(&self, locale: &str) -> Option<&str> {
        let val = match Self::normalize_locale(locale) {
            "en" => self.en.as_deref(),
            "zh-cn" => self.zh_cn.as_deref(),
            "zh-tw" => self.zh_tw.as_deref(),
            _ => None,
        };
        // `Some("")` 视同 None —— 空串不短路回退链
        match val {
            Some(s) if !s.is_empty() => Some(s),
            _ => None,
        }
    }

    /// 确定性回退链：请求语言 → 对象原文语言(lang) → en → zh-cn → zh-tw
    ///
    /// 返回 `(text, resolved_locale, is_fallback)`。
    /// 永不白屏——全部缺失时返回 `("", "en", true)`。
    pub fn get<'a>(&'a self, locale: &'a str, lang: &'a str) -> ResolvedText<'a> {
        let normalized_locale = Self::normalize_locale(locale);
        let normalized_lang = Self::normalize_locale(lang);

        // 回退链：locale → lang → en → zh-cn → zh-tw
        let chain = [normalized_locale, normalized_lang, "en", "zh-cn", "zh-tw"];
        let mut seen = std::collections::BTreeSet::new();
        for loc in chain {
            if !seen.insert(loc) {
                continue;
            }
            if let Some(text) = self.get_locale(loc) {
                let is_fallback = loc != normalized_locale;
                return ResolvedText {
                    text,
                    resolved_locale: loc,
                    is_fallback,
                };
            }
        }

        // 全部缺失 → 永不白屏
        ResolvedText {
            text: "",
            resolved_locale: "en",
            is_fallback: true,
        }
    }

    /// 验证：lang 指向的字段必须非空
    pub fn validate(&self, lang: &str) -> Result<(), String> {
        let normalized = Self::normalize_locale(lang);
        let ok = match normalized {
            "en" => self.en.as_deref().filter(|s| !s.is_empty()).is_some(),
            "zh-cn" => self.zh_cn.as_deref().filter(|s| !s.is_empty()).is_some(),
            "zh-tw" => self.zh_tw.as_deref().filter(|s| !s.is_empty()).is_some(),
            _ => return Err(format!("invalid lang: {}", lang)),
        };
        if ok {
            Ok(())
        } else {
            Err(format!(
                "lang '{}' field must be non-empty (en={:?}, zh_cn={:?}, zh_tw={:?})",
                lang, self.en, self.zh_cn, self.zh_tw
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field_with(en: Option<&str>, zh_cn: Option<&str>, zh_tw: Option<&str>) -> Localized {
        Localized {
            en: en.map(String::from),
            zh_cn: zh_cn.map(String::from),
            zh_tw: zh_tw.map(String::from),
        }
    }

    fn assert_resolved(r: ResolvedText, text: &str, locale: &str, is_fallback: bool) {
        assert_eq!(r.text, text, "text mismatch");
        assert_eq!(r.resolved_locale, locale, "locale mismatch");
        assert_eq!(r.is_fallback, is_fallback, "fallback mismatch");
    }

    #[test]
    fn test_zh_cn_exists() {
        let f = field_with(Some("AI"), Some("AI 市场"), None);
        let r = f.get("zh-cn", "en");
        assert_resolved(r, "AI 市场", "zh-cn", false);
    }

    #[test]
    fn test_zh_cn_empty_falls_to_en() {
        let f = field_with(Some("AI Market"), Some(""), None);
        let r = f.get("zh-cn", "en");
        assert_resolved(r, "AI Market", "en", true);
    }

    #[test]
    fn test_locale_missing_falls_to_lang() {
        // lang=zh-cn, 请求 zh-tw → zh-cn 存在
        let f = field_with(None, Some("AI 市场"), None);
        let r = f.get("zh-tw", "zh-cn");
        assert_resolved(r, "AI 市场", "zh-cn", true);
    }

    #[test]
    fn test_all_empty() {
        let f = field_with(None, None, None);
        let r = f.get("zh-cn", "en");
        assert_resolved(r, "", "en", true);
    }

    #[test]
    fn test_zh_cn_normalize() {
        let f = field_with(None, Some("AI 市场"), None);
        // zh-CN → zh-cn
        let r = f.get("zh-CN", "en");
        assert_resolved(r, "AI 市场", "zh-cn", false);
        // zh_cn → zh-cn
        let r = f.get("zh_cn", "en");
        assert_resolved(r, "AI 市场", "zh-cn", false);
    }

    #[test]
    fn test_validate_ok() {
        let f = field_with(Some("AI"), None, None);
        assert!(f.validate("en").is_ok());
    }

    #[test]
    fn test_validate_fail() {
        let f = field_with(Some(""), None, None);
        assert!(f.validate("en").is_err());
    }

    #[test]
    fn test_validate_wrong_lang() {
        let f = field_with(Some("AI"), None, None);
        assert!(f.validate("fr").is_err());
    }

    #[test]
    fn test_empty_string_treated_as_none() {
        // 关键：Some("") 不能短路 fallback
        let f = field_with(Some("AI Market"), Some(""), None);
        let r = f.get("zh-cn", "en");
        assert_resolved(r, "AI Market", "en", true);
    }

    #[test]
    fn test_en_only() {
        let f = Localized::en_only("AI Market");
        assert_eq!(f.en.as_deref(), Some("AI Market"));
        assert!(f.zh_cn.is_none());
        assert!(f.zh_tw.is_none());
    }

    #[test]
    fn test_fallback_to_any_non_empty() {
        // lang=en, en missing, zh-cn missing, zh-tw 存在
        let f = field_with(None, None, Some("AI 市場"));
        let r = f.get("en", "en");
        assert_resolved(r, "AI 市場", "zh-tw", true);
    }
}
