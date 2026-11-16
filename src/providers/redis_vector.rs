// Redis vector provider (feature `redis-vector`).
//
// A first-class provider over Redis that stores embeddings alongside the
// document and answers nearest-neighbour queries, so a caller gets vector
// search straight off the provider instead of wrapping it in
// `VectorSearchStorage` by hand.
//
// The scoring itself is NOT reimplemented here: `search_similar` builds a
// `VectorSearchStorage` over the same `RedisProvider` and delegates to it, so
// the distance maths, the sort and the top-k truncation stay in one tested
// place. Only the wiring — the four trait impls the contract requires —
// lives in this file.
//
// The embedding is stored as a JSON array in the document's embedding field,
// which is why `upsert_with_embedding` merges the vector into the document
// before handing it to Redis. That also means the store needs no RediSearch
// module: ranking happens in process, exactly as `VectorSearchStorage` does
// for JSON and SQLite. A provider-native HNSW/FLAT index is the coming-soon
// part (see docs/07-features/08c-vector-dbs.md).

use crate::error::OrmResult;
use crate::nosql_index::NosqlIndex;
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth, IndexInfo,
  ProviderCapabilities, RawResult, SchemaIntrospection, Storage, TransactionControl, TransactionId,
};
use crate::providers::redis::RedisProvider;
use crate::query::Filter;
use crate::vector_search::{DistanceMetric, VectorSearchStorage};
use async_trait::async_trait;
use serde_json::Value;

#[derive(Clone)]
pub struct RedisVectorProvider {
  inner: RedisProvider,
  embedding_field: String,
  metric: DistanceMetric,
}

impl RedisVectorProvider {
  pub async fn new(connection_string: impl AsRef<str>) -> OrmResult<Self> {
    Self::connect(connection_string).await
  }

  pub async fn connect(connection_string: impl AsRef<str>) -> OrmResult<Self> {
    let inner = RedisProvider::new(connection_string.as_ref()).await?;
    Ok(Self {
      inner,
      embedding_field: "_embedding".to_string(),
      metric: DistanceMetric::Cosine,
    })
  }

  pub async fn from_config(config: &crate::provider::ProviderConfig) -> OrmResult<Self> {
    Self::connect(&config.connection).await
  }

  pub fn with_prefix(mut self, prefix: &str) -> Self {
    self.inner = self.inner.with_prefix(prefix);
    self
  }

  pub fn with_embedding_field(mut self, field: impl Into<String>) -> Self {
    self.embedding_field = field.into();
    self
  }

  pub fn with_metric(mut self, metric: DistanceMetric) -> Self {
    self.metric = metric;
    self
  }

  // The vector view over the same connection. Built per call because
  // `RedisProvider` is cheap to clone (it shares the connection manager) and
  // this keeps a single owner for the scoring code.
  fn vectors(&self) -> VectorSearchStorage<RedisProvider> {
    VectorSearchStorage::new(self.inner.clone())
      .with_embedding_field(self.embedding_field.clone())
      .with_metric(self.metric)
  }

  // Store a document with its embedding merged in.
  pub async fn upsert_with_embedding(
    &self,
    collection: &str,
    doc: Value,
    embedding: Vec<f64>,
  ) -> OrmResult<Value> {
    self
      .vectors()
      .upsert_with_embedding(collection, doc, embedding)
      .await
  }

  // Top-k nearest documents as `(document, distance)` — lower is closer.
  pub async fn search_similar(
    &self,
    collection: &str,
    query_embedding: &[f64],
    top_k: usize,
  ) -> OrmResult<Vec<(Value, f64)>> {
    self
      .vectors()
      .search_similar(collection, query_embedding, top_k)
      .await
  }
}

#[async_trait]
impl Storage for RedisVectorProvider {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    self.inner.insert(collection, doc).await
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    self.inner.find_by_id(collection, id).await
  }

  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&Filter>,
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

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    self.inner.delete_many(collection, filter).await
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    self.inner.update_many(collection, filter, updates).await
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    self.inner.count(collection, filter).await
  }

  async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()> {
    self.inner.create_index(collection, index).await
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    self.inner.drop_index(collection, index_name).await
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Storage::list_indexes(&self.inner, collection).await
  }

  async fn aggregate(&self, collection: &str, pipeline: Vec<Value>) -> OrmResult<Vec<Value>> {
    self.inner.aggregate(collection, pipeline).await
  }

  async fn insert_many(&self, collection: &str, docs: Vec<Value>) -> OrmResult<usize> {
    self.inner.insert_many(collection, docs).await
  }

  async fn health_check(&self) -> OrmResult<bool> {
    self.inner.health_check().await
  }

  fn capabilities(&self) -> ProviderCapabilities {
    Storage::capabilities(&self.inner)
  }
}

#[async_trait]
impl SchemaIntrospection for RedisVectorProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    SchemaIntrospection::list_collections(&self.inner).await
  }

  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema> {
    self.inner.describe_collection(collection).await
  }

  async fn get_collection_stats(&self, collection: &str) -> OrmResult<CollectionStats> {
    self.inner.get_collection_stats(collection).await
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    SchemaIntrospection::list_indexes(&self.inner, collection).await
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    self.inner.get_database_name().await
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    self.inner.list_databases().await
  }
}

#[async_trait]
impl AdminCommands for RedisVectorProvider {
  async fn execute_raw(&self, sql: &str, params: Vec<Value>) -> OrmResult<RawResult> {
    self.inner.execute_raw(sql, params).await
  }

  async fn create_collection(&self, name: &str, schema: Option<CollectionSchema>) -> OrmResult<()> {
    self.inner.create_collection(name, schema).await
  }

  async fn drop_collection(&self, name: &str) -> OrmResult<()> {
    self.inner.drop_collection(name).await
  }

  async fn rename_collection(&self, old_name: &str, new_name: &str) -> OrmResult<()> {
    self.inner.rename_collection(old_name, new_name).await
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    self.inner.get_server_version().await
  }

  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    self.inner.health_check_detailed().await
  }
}

#[async_trait]
impl TransactionControl for RedisVectorProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    self.inner.begin_transaction().await
  }

  async fn commit_transaction(&self, id: TransactionId) -> OrmResult<()> {
    self.inner.commit_transaction(id).await
  }

  async fn rollback_transaction(&self, id: TransactionId) -> OrmResult<()> {
    self.inner.rollback_transaction(id).await
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    self.inner.is_transaction_active().await
  }
}
