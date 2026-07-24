//! Tag normalization layer — merges synonyms and enforces consistent spelling
//! before tags reach the database.
//!
//! This is a data-quality gate, not a classification system.  It handles
//! case, whitespace, acronyms, and common spelling variants.  It does NOT
//! do taxonomy (e.g. `Kubernetes → Cloud Infrastructure`).

/// Canonicalization rules: lowercase key → canonical display value.
///
/// Categories covered:
/// - **Acronyms**: `rce → Remote Code Execution`
/// - **Spelling**: `cyber security → Cybersecurity`
/// - **Brand casing**: `github → GitHub`
///
/// Not covered (require a taxonomy system, not this module):
/// - Semantic clustering (`Kubernetes → Cloud Infrastructure`)
/// - Entity extraction (`OpenAI → Organization`)
const TAG_CANONICAL_MAP: &[(&str, &str)] = &[
    // ---- Acronyms ----
    ("rce", "Remote Code Execution"),
    ("llm", "Large Language Models"),
    ("xss", "Cross-Site Scripting"),
    ("idp", "Identity Providers"),
    // ---- Spelling variants ----
    ("cyber security", "Cybersecurity"),
    ("cybersecurity", "Cybersecurity"),
    ("malware", "Malware"),
    ("ransomware", "Ransomware"),
    ("phishing", "Phishing"),
    ("botnet", "Botnet"),
    ("zeroday", "Zero-Day Vulnerabilities"),
    ("zero-day", "Zero-Day Vulnerabilities"),
    ("zero day", "Zero-Day Vulnerabilities"),
    ("zero-day exploits", "Zero-Day Vulnerabilities"),
    // ---- Brand casing ----
    ("github", "GitHub"),
    ("kubernetes", "Kubernetes"),
    ("docker", "Docker"),
    ("openai", "OpenAI"),
    ("microsoft 365", "Microsoft 365"),
    // ---- Common two-word compressions ----
    ("ai security", "AI Security"),
    ("ai safety", "AI Safety"),
    ("ai governance", "AI Governance"),
    ("network security", "Network Security"),
    ("cloud security", "Cloud Security"),
    ("cloud computing", "Cloud Computing"),
    ("machine learning", "Machine Learning"),
    ("supply chain", "Supply Chain"),
    ("identity security", "Identity Security"),
];

/// Normalize a list of tag strings: trim, filter empties,
/// canonicalize via map, deduplicate, sort.
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut result: Vec<String> = tags
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(canonicalize_tag)
        .collect();

    result.sort_unstable();
    result.dedup();
    result
}

/// Look up a single tag's canonical form.
/// Unknown tags pass through unchanged (trimmed).
fn canonicalize_tag(tag: &str) -> String {
    let key = tag.trim().to_lowercase();
    for &(k, v) in TAG_CANONICAL_MAP {
        if key == k {
            return v.to_string();
        }
    }
    tag.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_variants() {
        let input = vec!["AI Security".into(), "ai security".into(), "Ai Security".into()];
        let result = normalize_tags(&input);
        assert_eq!(result, vec!["AI Security"]);
    }

    #[test]
    fn expands_acronyms() {
        let input = vec!["RCE".into(), "rce".into(), "Remote Code Execution".into()];
        let result = normalize_tags(&input);
        assert_eq!(result, vec!["Remote Code Execution"]);
    }

    #[test]
    fn preserves_unknown_tags() {
        let input = vec!["Quantum Computing".into()];
        let result = normalize_tags(&input);
        assert_eq!(result, vec!["Quantum Computing"]);
    }

    #[test]
    fn trims_whitespace() {
        let input = vec!["  AI Security  ".into()];
        let result = normalize_tags(&input);
        assert_eq!(result, vec!["AI Security"]);
    }

    #[test]
    fn filters_empty_strings() {
        let input = vec!["".into(), "AI Security".into(), " ".into()];
        let result = normalize_tags(&input);
        assert_eq!(result, vec!["AI Security"]);
    }

    #[test]
    fn mixed_batch() {
        let input = vec!["RCE".into(), "AI security".into(), "rce".into()];
        let result = normalize_tags(&input);
        assert_eq!(result, vec!["AI Security", "Remote Code Execution"]);
    }

    #[test]
    fn empty_input() {
        let input: Vec<String> = vec![];
        let result = normalize_tags(&input);
        assert!(result.is_empty());
    }
}
