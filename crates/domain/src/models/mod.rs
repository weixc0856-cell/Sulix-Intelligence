pub(crate) mod artifact;
pub(crate) mod briefing;
pub(crate) mod claim;
pub(crate) mod confidence;
pub(crate) mod context_snapshot;
pub(crate) mod decision;
pub(crate) mod entity;
pub(crate) mod event;
pub(crate) mod feed;
pub(crate) mod memory;
pub(crate) mod observation;
pub(crate) mod outbox;
pub(crate) mod preview;
pub(crate) mod provenance;
pub(crate) mod reflection;
pub(crate) mod signal;
pub(crate) mod source;

pub use artifact::*;
pub use briefing::*;
pub use claim::*;
pub use confidence::*;
pub use context_snapshot::*;
pub use decision::*;
pub use entity::*;
pub use event::*;
pub use feed::*;
pub use memory::*;
pub use observation::*;
pub use outbox::*;
pub use preview::*;
pub use provenance::*;
pub use reflection::*;
pub use signal::*;
pub use source::*;

/// Deserialize a `bool` that may arrive across the JS/wasm bridge as a numeric
/// `0`/`1` (SQLite stores booleans as INTEGERs; D1 hands them to serde via
/// `serde_wasm_bindgen` as `0.0`/`1.0` f64, which a plain `bool` field cannot
/// accept and would otherwise panic on with "invalid type: floating point").
///
/// Accepts native bools and numeric 0/1 (any integer/float): `0`/`0.0` →
/// `false`, non-zero → `true`. Apply as:
/// `#[serde(deserialize_with = "crate::models::deserialize_bool_from_any")]`
pub(crate) fn deserialize_bool_from_any<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct BoolVisitor;

    impl serde::de::Visitor<'_> for BoolVisitor {
        type Value = bool;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a boolean or a numeric 0/1")
        }

        fn visit_bool<E>(self, v: bool) -> Result<bool, E> {
            Ok(v)
        }

        fn visit_i64<E>(self, v: i64) -> Result<bool, E> {
            Ok(v != 0)
        }

        fn visit_u64<E>(self, v: u64) -> Result<bool, E> {
            Ok(v != 0)
        }

        fn visit_f64<E>(self, v: f64) -> Result<bool, E> {
            Ok(v != 0.0)
        }
    }

    deserializer.deserialize_any(BoolVisitor)
}

#[cfg(test)]
mod tests {
    use super::deserialize_bool_from_any;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Row {
        #[serde(deserialize_with = "deserialize_bool_from_any")]
        flag: bool,
    }

    #[test]
    fn accepts_native_bool() {
        let r: Row = serde_json::from_str(r#"{"flag":true}"#).unwrap();
        assert!(r.flag);
        let r: Row = serde_json::from_str(r#"{"flag":false}"#).unwrap();
        assert!(!r.flag);
    }

    #[test]
    fn accepts_integer_0_1() {
        let r: Row = serde_json::from_str(r#"{"flag":1}"#).unwrap();
        assert!(r.flag);
        let r: Row = serde_json::from_str(r#"{"flag":0}"#).unwrap();
        assert!(!r.flag);
    }

    #[test]
    fn accepts_float_0_1() {
        // D1 / serde_wasm_bindgen delivers SQLite INTEGERs as f64 (0.0/1.0).
        let r: Row = serde_json::from_str(r#"{"flag":1.0}"#).unwrap();
        assert!(r.flag);
        let r: Row = serde_json::from_str(r#"{"flag":0.0}"#).unwrap();
        assert!(!r.flag);
    }
}
