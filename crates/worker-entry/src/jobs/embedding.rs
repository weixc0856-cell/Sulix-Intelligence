use vectorize::{VectorMetadata, VectorRecord, VectorizeIndex};

/// Upsert an article embedding into the Vectorize index.
pub(crate) async fn upsert_vector(idx: &VectorizeIndex, article_id: i64, embedding: &[f32]) -> Result<(), String> {
    let record = VectorRecord {
        id: format!("article-{article_id}"),
        values: embedding.to_vec(),
        metadata: Some(VectorMetadata { article_id, feed_id: None, published_at: None }),
    };
    vectorize::upsert_vector(idx, &record).await
}
