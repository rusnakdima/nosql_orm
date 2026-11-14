// Vector / semantic search: store embeddings and retrieve by similarity.
//
// Wraps any `Storage` and adds vector search capabilities. Embeddings are
// stored in a configurable field (default "_embedding") as Vec<f64>.

use crate::error::OrmResult;
use crate::provider::Storage;
use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistanceMetric {
  Cosine,
  Euclidean,
  DotProduct,
}

impl DistanceMetric {
  pub fn distance(&self, a: &[f64], b: &[f64]) -> f64 {
    match self {
      DistanceMetric::Cosine => {
        let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
        if na == 0.0 || nb == 0.0 {
          return 1.0;
        }
        1.0 - (dot / (na * nb))
      }
      DistanceMetric::Euclidean => a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt(),
      DistanceMetric::DotProduct => {
        let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        -dot // negate so lower = more similar
      }
    }
  }
}

#[derive(Clone)]
pub struct VectorSearchStorage<P: Storage> {
  inner: P,
  embedding_field: String,
  metric: DistanceMetric,
}

impl<P: Storage> VectorSearchStorage<P> {
  pub fn new(inner: P) -> Self {
    Self {
      inner,
      embedding_field: "_embedding".to_string(),
      metric: DistanceMetric::Cosine,
    }
  }

  pub fn with_embedding_field(mut self, field: impl Into<String>) -> Self {
    self.embedding_field = field.into();
    self
  }

  pub fn with_metric(mut self, metric: DistanceMetric) -> Self {
    self.metric = metric;
    self
  }

  // Store a document with its embedding.
  pub async fn upsert_with_embedding(
    &self,
    collection: &str,
    doc: Value,
    embedding: Vec<f64>,
  ) -> OrmResult<Value> {
    let mut doc = doc;
    if let Some(obj) = doc.as_object_mut() {
      obj.insert(self.embedding_field.clone(), serde_json::json!(embedding));
    }
    self.inner.insert(collection, doc).await
  }

  // Search for the top-k most similar documents.
  pub async fn search_similar(
    &self,
    collection: &str,
    query_embedding: &[f64],
    top_k: usize,
  ) -> OrmResult<Vec<(Value, f64)>> {
    let docs = self.inner.find_all(collection).await?;
    let mut scored: Vec<(Value, f64)> = docs
      .into_iter()
      .filter_map(|doc| {
        let emb = doc
          .get(&self.embedding_field)
          .and_then(|v| v.as_array())
          .map(|arr| arr.iter().filter_map(|x| x.as_f64()).collect::<Vec<f64>>())?;
        if emb.len() != query_embedding.len() {
          return None;
        }
        let dist = self.metric.distance(&emb, query_embedding);
        Some((doc, dist))
      })
      .collect();
    scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);
    Ok(scored)
  }
}

#[async_trait]
impl<P: Storage> Storage for VectorSearchStorage<P> {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    self.inner.insert(collection, doc).await
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    self.inner.find_by_id(collection, id).await
  }

  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&crate::query::Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<&str>,
    sort_asc: bool,
  ) -> OrmResult<Vec<Value>> {
    self
      .inner
      .find_many(collection, filter, skip, limit, sort_by, sort_asc)
      .await
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    self.inner.update(collection, id, doc).await
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    self.inner.patch(collection, id, patch).await
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    self.inner.delete(collection, id).await
  }

  async fn delete_many(&self, collection: &str, filter: Option<crate::query::Filter>) -> OrmResult<usize> {
    self.inner.delete_many(collection, filter).await
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<crate::query::Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    self.inner.update_many(collection, filter, updates).await
  }

  async fn count(&self, collection: &str, filter: Option<&crate::query::Filter>) -> OrmResult<u64> {
    self.inner.count(collection, filter).await
  }

  async fn create_index(&self, collection: &str, index: &crate::nosql_index::NosqlIndex) -> OrmResult<()> {
    self.inner.create_index(collection, index).await
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    self.inner.drop_index(collection, index_name).await
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<crate::provider::IndexInfo>> {
    crate::provider::Storage::list_indexes(&self.inner, collection).await
  }

  async fn aggregate(&self, collection: &str, pipeline: Vec<Value>) -> OrmResult<Vec<Value>> {
    self.inner.aggregate(collection, pipeline).await
  }

  async fn insert_many(&self, collection: &str, docs: Vec<Value>) -> OrmResult<usize> {
    self.inner.insert_many(collection, docs).await
  }
}