//! Typed wrapper around Cloudflare Vectorize.
//!
//! The underlying binding is accessed via raw `#[wasm_bindgen]` because
//! `worker-0.8` doesn't ship a native Vectorize type.  Once it does, swap
//! the implementation here without touching business logic.
//!
//! Currently supports upsert, query, and delete.  Query is used by the
//! semantic search endpoint; delete is used for test cleanup.

use worker::wasm_bindgen;
use worker::wasm_bindgen::prelude::*;
use worker::wasm_bindgen::JsCast;
use worker::wasm_bindgen_futures;
use worker::EnvBinding;

use js_sys::{Array, Float32Array, Object, Reflect};

// ---- Raw binding (kept in one place) ----

/// Cloudflare Vectorize index binding.
#[wasm_bindgen]
extern "C" {
    pub type VectorizeIndex;

    #[wasm_bindgen(method, catch)]
    async fn upsert(this: &VectorizeIndex, vectors: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn query(this: &VectorizeIndex, vector: JsValue, opts: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn delete(this: &VectorizeIndex, ids: JsValue) -> Result<JsValue, JsValue>;
}

impl EnvBinding for VectorizeIndex {
    const TYPE_NAME: &'static str = "VectorizeIndex";
}

// ---- Typed wrapper ----

/// A single vector to upsert into the index.
pub struct VectorEntry {
    pub id: String,
    pub values: Vec<f32>,
    pub metadata: Option<serde_json::Value>,
}

/// A match returned from Vectorize query.
#[derive(Debug, Clone)]
pub struct VectorMatch {
    pub id: String,
    pub score: f32,
}

/// Upsert a batch of vectors into the Vectorize index.
pub async fn upsert_vectors(
    index: &VectorizeIndex,
    entries: &[VectorEntry],
) -> Result<u64, String> {
    let vectors = Array::new();

    for entry in entries {
        let vec_obj = Object::new();
        let _ = Reflect::set(&vec_obj, &"id".into(), &entry.id.clone().into());

        let values = Float32Array::new_with_length(entry.values.len() as u32);
        for (i, v) in entry.values.iter().enumerate() {
            values.set_index(i as u32, *v);
        }
        let _ = Reflect::set(&vec_obj, &"values".into(), &values.into());

        if let Some(ref meta) = entry.metadata {
            let meta_obj = meta_value_to_js(meta);
            let _ = Reflect::set(&vec_obj, &"metadata".into(), &meta_obj);
        }

        vectors.push(&vec_obj);
    }

    index
        .upsert(vectors.into())
        .await
        .map(|_| entries.len() as u64)
        .map_err(|e| format!("{e:?}"))
}

/// Query the Vectorize index for nearest neighbors.
pub async fn query_vectors(
    index: &VectorizeIndex,
    embedding: &[f32],
    top_k: u32,
) -> Result<Vec<VectorMatch>, String> {
    let values = Float32Array::new_with_length(embedding.len() as u32);
    for (i, v) in embedding.iter().enumerate() {
        values.set_index(i as u32, *v);
    }
    let vector = Object::new();
    let _ = Reflect::set(&vector, &"vector".into(), &values.into());

    let opts = Object::new();
    let _ = Reflect::set(&opts, &"topK".into(), &JsValue::from_f64(top_k as f64));
    let _ = Reflect::set(&opts, &"returnMetadata".into(), &JsValue::from_bool(false));

    let result = index
        .query(vector.into(), opts.into())
        .await
        .map_err(|e| format!("{e:?}"))?;

    let matches = Reflect::get(&result, &"matches".into())
        .map_err(|e| format!("missing matches: {e:?}"))?
        .dyn_into::<Array>()
        .map_err(|_| "matches not array".to_string())?;

    let mut results = Vec::new();
    for i in 0..matches.length() {
        let item = matches.get(i);
        let id = Reflect::get(&item, &"id".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let score = Reflect::get(&item, &"score".into())
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        results.push(VectorMatch { id, score });
    }
    Ok(results)
}

/// Delete vectors by their IDs.
#[allow(dead_code)]
pub async fn delete_vectors(index: &VectorizeIndex, ids: &[String]) -> Result<(), String> {
    let js_arr = Array::new();
    for id in ids {
        js_arr.push(&id.clone().into());
    }
    index
        .delete(js_arr.into())
        .await
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

// ---- (deprecated) Fire-and-forget upsert — replaced by awaited upsert_vectors ----

pub fn upsert_vector_faf(index: &VectorizeIndex, article_id: i64, embedding: &[f32]) {
    let vec_obj = Object::new();
    let _ = Reflect::set(&vec_obj, &"id".into(), &format!("article-{article_id}").into());
    let values = Float32Array::new_with_length(embedding.len() as u32);
    for (i, v) in embedding.iter().enumerate() {
        values.set_index(i as u32, *v);
    }
    let _ = Reflect::set(&vec_obj, &"values".into(), &values.into());
    let metadata = Object::new();
    let _ = Reflect::set(&metadata, &"article_id".into(), &wasm_bindgen::JsValue::from_f64(article_id as f64));
    let _ = Reflect::set(&vec_obj, &"metadata".into(), &metadata.into());
    let vectors = Array::new();
    vectors.push(&vec_obj);
    let vectors_js: JsValue = vectors.into();
    let js_val: &JsValue = index.as_ref();
    let idx_owned: VectorizeIndex = JsCast::unchecked_into(js_val.clone());
    wasm_bindgen_futures::spawn_local(async move {
        match idx_owned.upsert(vectors_js).await {
            Ok(_) => {}
            Err(e) => worker::console_log!("  vectorize upsert failed for article {article_id}: {e:?}"),
        }
    });
}

// ---- Helpers ----

fn meta_value_to_js(v: &serde_json::Value) -> JsValue {
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
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;

    #[test]
    fn json_null_to_js() { let r = meta_value_to_js(&serde_json::Value::Null); assert!(r.is_null()); }
    #[test]
    fn json_bool_to_js() { let r = meta_value_to_js(&serde_json::Value::Bool(true)); assert_eq!(r.as_bool(), Some(true)); }
    #[test]
    fn json_number_to_js() { let r = meta_value_to_js(&serde_json::json!(42.5)); assert!(!r.is_undefined()); }
    #[test]
    fn json_string_to_js() { let r = meta_value_to_js(&serde_json::Value::String("hello".into())); assert_eq!(r.as_string(), Some("hello".into())); }
    #[test]
    fn json_array_to_js() { let r = meta_value_to_js(&serde_json::json!([1, "two", false])); assert!(r.is_object() || r.is_array()); }
    #[test]
    fn json_object_to_js() { let r = meta_value_to_js(&serde_json::json!({"a": 1, "b": "hello"})); assert!(r.is_object()); }
}
