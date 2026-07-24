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

use js_sys::{Array, Object, Reflect};

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

#[cfg(test)]
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
    fn json_string_to_js() {
        let r = meta_value_to_js(&serde_json::Value::String("hello".into()));
        assert_eq!(r.as_string(), Some("hello".into()));
    }
}
