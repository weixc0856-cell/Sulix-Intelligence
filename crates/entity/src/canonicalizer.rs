//! Entity name canonicalization — deduplicate "OpenAI" vs "Open AI" vs "open-ai".
//!
//! The canonicalizer provides two operations:
//! - `normalize()` — lossy normalisation for dedup (lowercase + strip all non-alpha).
//! - `canonicalize()` — map common spelling variants to a canonical form.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Known spelling variants mapped to canonical form.
///
/// Keys MUST be in *normalised* form (lowercase, all non-alphanumeric stripped)
/// so `canonicalize()` can look them up directly after normalising the input.
static CANONICAL_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // AI organisations
    m.insert("openai", "OpenAI");
    m.insert("anthropicai", "Anthropic");
    m.insert("googledeepmind", "DeepMind");
    m.insert("metaai", "Meta");
    // NVIDIA
    m.insert("nvidia", "NVIDIA");
    m.insert("nvidiacorp", "NVIDIA");
    m.insert("nvidiacorporation", "NVIDIA");
    // AMD
    m.insert("amd", "AMD");
    m.insert("advancedmicrodevices", "AMD");
    // Google / Meta / Microsoft
    m.insert("google", "Google");
    m.insert("googleai", "Google");
    m.insert("meta", "Meta");
    m.insert("facebook", "Meta");
    m.insert("microsoft", "Microsoft");
    m.insert("msft", "Microsoft");
    // Apple
    m.insert("apple", "Apple");
    m.insert("appleinc", "Apple");
    // Amazon
    m.insert("amazon", "Amazon");
    m.insert("amazonwebservices", "AWS");
    m.insert("aws", "AWS");
    m.insert("googlecloud", "GCP");
    m.insert("microsoftazure", "Azure");
    // OpenAI products
    m.insert("gpt5", "GPT-5");
    m.insert("gpt4", "GPT-4");
    m.insert("gpt4o", "GPT-4o");
    // ML / infra
    m.insert("pytorch", "PyTorch");
    m.insert("tensorflow", "TensorFlow");
    m.insert("llama", "Llama");
    m.insert("kubernetes", "Kubernetes");
    m.insert("k8s", "Kubernetes");
    m.insert("gcp", "GCP");
    m.insert("azure", "Azure");
    // RCE
    m.insert("rce", "RCE");
    m.insert("remotecodeexecution", "RCE");
    m.insert("remotecodeexecutionrce", "RCE");
    m
});

/// Normalise a name for dedup lookups: lowercase, strip all non-alphanumeric
/// including whitespace. "OpenAI", "Open AI", and "open-ai" all yield "openai".
///
/// # Examples
/// ```
/// use entity::canonicalizer::normalize;
/// assert_eq!(normalize("OpenAI"), normalize("Open AI"));
/// assert_eq!(normalize("open-ai"), normalize("openai"));
/// ```
pub fn normalize(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Return the canonical display name for `raw_name`, or a cleaned-up version
/// if no mapping exists.
///
/// This first normalises the input, looks up the known map, and falls back
/// to the input with leading/trailing whitespace trimmed.
pub fn canonicalize(raw_name: &str) -> String {
    let key = normalize(raw_name);
    CANONICAL_MAP
        .get(key.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| raw_name.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_deduplicates_variants() {
        assert_eq!(normalize("OpenAI"), "openai");
        assert_eq!(normalize("Open AI"), "openai");
        assert_eq!(normalize("open-ai"), "openai");
        assert_eq!(normalize("open_ai"), "openai");
    }

    #[test]
    fn normalize_strips_all_whitespace() {
        assert_eq!(normalize("NVIDIA's GPU"), "nvidiasgpu");
    }

    #[test]
    fn canonicalize_known_variants() {
        assert_eq!(canonicalize("OpenAI"), "OpenAI");
        assert_eq!(canonicalize("Open AI"), "OpenAI");
        assert_eq!(canonicalize("open-ai"), "OpenAI");
        assert_eq!(canonicalize("nvidia corp"), "NVIDIA");
        assert_eq!(canonicalize("NVIDIA Corporation"), "NVIDIA");
    }

    #[test]
    fn canonicalize_fallback() {
        assert_eq!(canonicalize("Unknown Corp"), "Unknown Corp");
        assert_eq!(canonicalize("  trimmed  "), "trimmed");
    }

    #[test]
    fn canonicalize_rce_variants_all_map_to_rce() {
        assert_eq!(canonicalize("RCE"), "RCE");
        assert_eq!(canonicalize("Remote Code Execution"), "RCE");
        assert_eq!(canonicalize("remote code execution (RCE)"), "RCE");
    }

    #[test]
    fn canonicalize_rce() {
        assert_eq!(canonicalize("RCE"), "RCE");
        assert_eq!(canonicalize("remote code execution"), "RCE");
        assert_eq!(canonicalize("Remote Code Execution (RCE)"), "RCE");
    }
}
