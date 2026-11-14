// pgvector provider: reuses PostgresProvider with vector search support.
//
// Requires the `pgvector` extension in PostgreSQL. The provider is a thin
// wrapper around PostgresProvider that adds vector similarity search.

use crate::error::OrmResult;
use crate::provider::Storage;
use crate::providers::sql::postgres::PostgresProvider;
use async_trait::async_trait;
use serde_json::Value;

#[derive(Clone)]
pub struct PgVectorProvider {
  inner: PostgresProvider,
  embedding_field: String,
}

impl PgVectorProvider {
  pub async fn connect(uri: impl AsRef<str>) -> OrmResult<Self> {
    let inner = PostgresProvider::connect(uri).await?;
    Ok(Self {
      inner,
      embedding_field: "_embedding".to_string(),
    })
  }

  pub fn with_embedding_field(mut self, field: impl Into<String>) -> Self {
    self.embedding_field = field.into();
    self
  }

  // Create the vector extension and index.
  pub async fn setup_vector(&self) -> OrmResult<()> {
    crate::provider::AdminCommands::execute_raw(
      &self.inner,
      "CREATE EXTENSION IF NOT EXISTS vector",
      vec![],
    )
    .await?;
    Ok(())
  }

  // Search for similar vectors using cosine distance.
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
        let dot: f64 = emb.iter().zip(query_embedding).map(|(a, b)| a * b).sum();
        let na: f64 = emb.iter().map(|x| x * x).sum::<f64>().sqrt();
        let nb: f64 = query_embedding.iter().map(|x| x * x).sum::<f64>().sqrt();
        let dist = if na == 0.0 || nb == 0.0 {
          1.0
        } else {
          1.0 - (dot / (na * nb))
        };
        Some((doc, dist))
      })
      .collect();
    scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);
    Ok(scored)
  }
}

#[async_trait]
impl Storage for PgVectorProvider {
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

// TransactionControl delegates to inner
#[async_trait]
impl crate::provider::TransactionControl for PgVectorProvider {
  async fn begin_transaction(&self) -> OrmResult<crate::provider::TransactionId> {
    self.inner.begin_transaction().await
  }

  async fn commit_transaction(&self, id: crate::provider::TransactionId) -> OrmResult<()> {
    self.inner.commit_transaction(id).await
  }

  async fn rollback_transaction(&self, id: crate::provider::TransactionId) -> OrmResult<()> {
    self.inner.rollback_transaction(id).await
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    self.inner.is_transaction_active().await
  }
}