//! Shared utilities for the worker-entry crate.

/// Truncate a &str reference to `max_chars` characters at UTF-8 safe boundaries.
pub fn truncate_chars(input: &str, max_chars: usize) -> &str {
    input.char_indices().nth(max_chars).map(|(idx, _)| &input[..idx]).unwrap_or(input)
}

/// Truncate a string body to `max_chars` characters, returning an owned String.
///
/// Unlike [`truncate_chars`] which borrows, this is suitable for storing
/// error excerpts that outlive the original response body.
pub fn truncate_body(body: &str, max_chars: usize) -> String {
    body.chars().take(max_chars).collect()
}
