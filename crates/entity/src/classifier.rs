//! Heuristic entity-type classification.
//!
//! Uses pattern matching and known-entity lists rather than ML to avoid
//! extra LLM calls.  Accuracy can be upgraded to ML-based classification
//! in a later phase without changing the caller interface.

/// Known organisation names — mapped to `organization` entity type.
const KNOWN_ORGANIZATIONS: &[&str] = &[
    "OpenAI",
    "Google",
    "Microsoft",
    "Apple",
    "Meta",
    "Amazon",
    "TSMC",
    "NVIDIA",
    "AMD",
    "Intel",
    "IBM",
    "Tesla",
    "SpaceX",
    "ByteDance",
    "Tencent",
    "Alibaba",
    "DeepMind",
    "Anthropic",
    "Hugging Face",
    "Mistral AI",
    "Cohere",
    "Stability AI",
    "Midjourney",
    "Cloudflare",
    "Netflix",
    "Spotify",
    "Uber",
    "Airbnb",
    "Stripe",
    "Salesforce",
    "Oracle",
    "SAP",
    "Samsung",
    "Sony",
    "Qualcomm",
    "ARM",
    "ASML",
];

/// Known product / project names — mapped to `product` entity type.
const KNOWN_PRODUCTS: &[&str] = &[
    "GPT-5",
    "GPT-4",
    "GPT-4o",
    "Claude",
    "Gemini",
    "Gemma",
    "Llama",
    "Mistral",
    "Kubernetes",
    "Docker",
    "PyTorch",
    "TensorFlow",
    "JAX",
    "CUDA",
    "ROCm",
    "WebGPU",
    "WebAssembly",
    "Rust",
    "Python",
    "TypeScript",
    "React",
    "Next.js",
    "Astro",
    "Redis",
    "PostgreSQL",
    "SQLite",
    "Kafka",
    "Spark",
    "Flink",
    "Ray",
    "vLLM",
    "Triton",
    "LangChain",
    "AutoGPT",
    "iPhone",
    "Android",
    "Windows",
    "Linux",
    "macOS",
    "AWS",
    "GCP",
    "Azure",
];

/// Company / legal entity suffixes that indicate the name is an organization.
const COMPANY_SUFFIXES: &[&str] = &["Inc", "Corp", "LLC", "Ltd", "GmbH", "Co", "Corporation", "Incorporated"];

/// Classify an entity name into a type string.
///
/// Returns one of: `"vulnerability"`, `"organization"`, `"product"`, `"unknown"`.
pub fn classify(name: &str) -> &'static str {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "unknown";
    }

    // CVE pattern
    if trimmed.starts_with("CVE-") {
        return "vulnerability";
    }

    // Check known organisations first
    if KNOWN_ORGANIZATIONS.iter().any(|k| eq_ignore_case(trimmed, k)) {
        return "organization";
    }

    // Check known products
    if KNOWN_PRODUCTS.iter().any(|k| eq_ignore_case(trimmed, k)) {
        return "product";
    }

    // Company suffix
    if COMPANY_SUFFIXES.iter().any(|suf| trimmed.ends_with(suf)) {
        return "organization";
    }

    // All-caps short name (likely a product/company acronym)
    if trimmed.len() >= 3 && trimmed.len() <= 8 && trimmed.chars().all(|c| c.is_uppercase() || c.is_ascii_digit()) {
        return "product";
    }

    "unknown"
}

fn eq_ignore_case(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cve_is_vulnerability() {
        assert_eq!(classify("CVE-2026-1234"), "vulnerability");
        assert_eq!(classify("CVE-2024-12345"), "vulnerability");
        assert_eq!(classify("CVE-2023-0001"), "vulnerability");
    }

    #[test]
    fn known_organizations() {
        assert_eq!(classify("OpenAI"), "organization");
        assert_eq!(classify("Google"), "organization");
        assert_eq!(classify("NVIDIA"), "organization");
        assert_eq!(classify("TSMC"), "organization");
        assert_eq!(classify("Hugging Face"), "organization");
    }

    #[test]
    fn known_products() {
        assert_eq!(classify("Kubernetes"), "product");
        assert_eq!(classify("Docker"), "product");
        assert_eq!(classify("PyTorch"), "product");
        assert_eq!(classify("CUDA"), "product");
    }

    #[test]
    fn company_suffix() {
        assert_eq!(classify("SomeCorp"), "organization");
        assert_eq!(classify("Tech Inc"), "organization");
        assert_eq!(classify("Data GmbH"), "organization");
    }

    #[test]
    fn all_caps_acronym() {
        assert_eq!(classify("AWS"), "product");
        assert_eq!(classify("GCP"), "product");
        // ARM is known as an organization, so classify returns that
        assert_eq!(classify("XYZ"), "product");
        assert_eq!(classify("API"), "product");
    }

    #[test]
    fn unknown_returns_default() {
        assert_eq!(classify("Quantum Computing"), "unknown");
        assert_eq!(classify(""), "unknown");
        assert_eq!(classify("   "), "unknown");
    }

    #[test]
    fn case_insensitive_known_lists() {
        assert_eq!(classify("openai"), "organization");
        assert_eq!(classify("nvidia"), "organization");
        assert_eq!(classify("kubernetes"), "product");
        assert_eq!(classify("pytorch"), "product");
    }
}
