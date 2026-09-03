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

/// Embed an article's summary via the composition-root `Embedder`, then upsert
/// the vector. Returns Err (logged, never fatal) on embed OR upsert failure —
/// by the time this is called the article + summary are already persisted, so
/// the article is merely absent from Vectorize and re-embeddable by the admin
/// rebuild endpoint.
pub(crate) async fn embed_and_upsert(
    embedder: &dyn ai_pipeline::Embedder,
    idx: &VectorizeIndex,
    article_id: i64,
    title: &str,
    summary: &str,
    raw_tags: &[String],
) -> Result<(), String> {
    // Normalize tags exactly like process_article persists them, so the text
    // embedded at ingestion time == the text the admin rebuild endpoint embeds
    // from the stored ai_tags row == the query-time text contract.
    let tags = ai_pipeline::tag_normalizer::normalize_tags(raw_tags);
    let text = embedding::build_embedding_text(title, summary, &tags, None);
    let embedding = embedder.embed(&text).await.map_err(|e| format!("embed failed: {e}"))?;
    upsert_vector(idx, article_id, &embedding).await
}
