/// Single source of truth for pipeline version metadata.
///
/// Bump this constant when the pipeline changes in a way that affects
/// artifact compatibility (schema changes, new model defaults, etc.).
///
/// Format: MAJOR.MINOR.PATCH (semantic versioning).
/// - MAJOR: breaking change to artifact schema
/// - MINOR: new artifact type or backward-compatible schema extension
/// - PATCH: metadata-only change (model name, description, etc.)
pub const PIPELINE_VERSION: &str = "0.1.0";
