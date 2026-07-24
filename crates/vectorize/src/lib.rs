//! Cloudflare Vectorize index binding.
//!
//! Exposes the raw `#[wasm_bindgen]` extern type for `VectorizeIndex`
//! because `worker-0.8` doesn't ship a native Vectorize type.  Once it
//! does, swap the implementation here without touching any consumers.
//!
//! All three operations (upsert, query, delete) are declared in one
//! place so downstream crates never duplicate the extern block.

use worker::wasm_bindgen::prelude::*;
use worker::EnvBinding;

/// Cloudflare Vectorize index binding.
#[wasm_bindgen]
extern "C" {
    pub type VectorizeIndex;

    #[wasm_bindgen(method, catch)]
    pub async fn upsert(this: &VectorizeIndex, vectors: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    pub async fn query(this: &VectorizeIndex, vector: JsValue, opts: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    pub async fn delete(this: &VectorizeIndex, ids: JsValue) -> Result<JsValue, JsValue>;
}

impl EnvBinding for VectorizeIndex {
    const TYPE_NAME: &'static str = "VectorizeIndexImpl";
}

use js_sys::{Array, Float32Array, Object, Reflect};

/// Convert a `serde_json::Value` to a `JsValue` for use with Vectorize metadata.
/// Recursively handles Null, Bool, Number, String, Array, and Object.
pub fn meta_value_to_js(v: &serde_json::Value) -> JsValue {
    match v {
        serde_json::Value::Null => JsValue::null(),
        serde_json::Value::Bool(b) => JsValue::from_bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                JsValue::from_f64(f)
            } else {
                JsValue::from_str(&n.to_string())
            }
        }
        serde_json::Value::String(s) => JsValue::from_str(s),
        serde_json::Value::Array(arr) => {
            let js_arr = Array::new();
            for item in arr {
                js_arr.push(&meta_value_to_js(item));
            }
            js_arr.into()
        }
        serde_json::Value::Object(map) => {
            let obj = Object::new();
            for (k, v) in map {
                let _ = Reflect::set(&obj, &k.clone().into(), &meta_value_to_js(v));
            }
            obj.into()
        }
    }
}

/// A single vector record to be upserted into a Vectorize index.
pub struct VectorRecord {
    /// Namespaced ID, e.g. "article-{id}".
    pub id: String,
    /// Float32 embedding values.
    pub values: Vec<f32>,
    /// Optional metadata attached to the vector.
    pub metadata: Option<VectorMetadata>,
}

/// Structured metadata for a Vectorize vector.
pub struct VectorMetadata {
    pub article_id: i64,
    pub feed_id: Option<i64>,
    pub published_at: Option<i64>,
}

/// Upsert a single embedding vector to a Vectorize index.
///
/// Builds a Float32Array from `record.values`, attaches metadata
/// as a JS Object when present, then delegates to `idx.upsert()`.
pub async fn upsert_vector(
    idx: &VectorizeIndex,
    record: &VectorRecord,
) -> Result<(), String> {
    let vec_obj = Object::new();
    let _ = Reflect::set(&vec_obj, &"id".into(), &record.id.clone().into());

    let values = Float32Array::new_with_length(record.values.len() as u32);
    for (i, v) in record.values.iter().enumerate() {
        values.set_index(i as u32, *v);
    }
    let _ = Reflect::set(&vec_obj, &"values".into(), &values.into());

    if let Some(ref meta) = record.metadata {
        let meta_obj = Object::new();
        let _ = Reflect::set(&meta_obj, &"article_id".into(), &JsValue::from_f64(meta.article_id as f64));
        if let Some(fid) = meta.feed_id {
            let _ = Reflect::set(&meta_obj, &"feed_id".into(), &JsValue::from_f64(fid as f64));
        }
        if let Some(ts) = meta.published_at {
            let _ = Reflect::set(&meta_obj, &"published_at".into(), &JsValue::from_f64(ts as f64));
        }
        let _ = Reflect::set(&vec_obj, &"metadata".into(), &meta_obj.into());
    }

    let vectors = Array::new();
    vectors.push(&vec_obj);
    idx.upsert(vectors.into()).await.map(|_| ()).map_err(|e| format!("{e:?}"))
}

#[cfg(test)]
#[cfg(target_arch = "wasm32")]
mod tests {
    use super::*;

    #[test]
    fn json_null_to_js() {
        let r = meta_value_to_js(&serde_json::Value::Null);
        assert!(r.is_null());
    }
    #[test]
    fn json_bool_to_js() {
        let r = meta_value_to_js(&serde_json::Value::Bool(true));
        assert_eq!(r.as_bool(), Some(true));
    }
    #[test]
    fn json_number_to_js() {
        let r = meta_value_to_js(&serde_json::json!(42.5));
        assert!(!r.is_undefined());
    }
    #[test]
    fn json_string_to_js() {
        let r = meta_value_to_js(&serde_json::Value::String("hello".into()));
        assert_eq!(r.as_string(), Some("hello".into()));
    }
    #[test]
    fn json_array_to_js() {
        let r = meta_value_to_js(&serde_json::json!([1, "two", false]));
        assert!(r.is_object() || r.is_array());
    }
    #[test]
    fn json_object_to_js() {
        let r = meta_value_to_js(&serde_json::json!({"a": 1, "b": "hello"}));
        assert!(r.is_object());
    }
}
