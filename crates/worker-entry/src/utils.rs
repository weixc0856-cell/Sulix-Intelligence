//! Shared utilities for the worker-entry crate.

/// Truncate a string to `max_chars` characters at UTF-8 safe boundaries.
///
/// Uses `char_indices()` to avoid the panic that `&input[..N]` causes when
/// the byte index falls in the middle of a multi-byte character.
///
/// # Example
/// ```
/// assert_eq!(truncate_chars("hello world", 5), "hello");
/// assert_eq!(truncate_chars("你好世界", 3), "你好世");
/// assert_eq!(truncate_chars("abc", 10), "abc");  // shorter than max
/// ```
pub fn truncate_chars(input: &str, max_chars: usize) -> &str {
    input.char_indices().nth(max_chars).map(|(idx, _)| &input[..idx]).unwrap_or(input)
}
