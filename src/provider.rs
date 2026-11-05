use crate::error::{OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::query::Filter;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub use crate::admin_types::{
  CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth, FieldInfo, IndexInfo,
  PoolStats, RawResult, TransactionId,
};

// A capability is an operation the provider may implement. Callers ask before
// calling; a provider that does not implement one returns NotSupported instead
// of pretending the call succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
  Transactions,
  Indexes,
  Aggregation,
  RawQueries,
}

impl Capability {
  pub const ALL: [Capability; 4] = [
    Capability::Transactions,
    Capability::Indexes,
    Capability::Aggregation,
    Capability::RawQueries,
  ];

  pub fn as_str(self) -> &'static str {
    match self {
      Capability::Transactions => "transactions",
      Capability::Indexes => "indexes",
      Capability::Aggregation => "aggregation",
      Capability::RawQueries => "raw_queries",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProviderCapabilities {
  transactions: bool,
  indexes: bool,
  aggregation: bool,
  raw_queries: bool,
}

impl ProviderCapabilities {
  // The conservative default: a provider that says nothing supports nothing.
  pub const fn none() -> Self {
    Self {
      transactions: false,
      indexes: false,
      aggregation: false,
      raw_queries: false,
    }
  }

  pub const fn all() -> Self {
    Self {
      transactions: true,
      indexes: true,
      aggregation: true,
      raw_queries: true,
    }
  }

  pub const fn with(self, capability: Capability) -> Self {
    let mut caps = self;
    match capability {
      Capability::Transactions => caps.transactions = true,
      Capability::Indexes => caps.indexes = true,
      Capability::Aggregation => caps.aggregation = true,
      Capability::RawQueries => caps.raw_queries = true,
    }
    caps
  }

  pub const fn supports(&self, capability: Capability) -> bool {
    match capability {
      Capability::Transactions => self.transactions,
      Capability::Indexes => self.indexes,
      Capability::Aggregation => self.aggregation,
      Capability::RawQueries => self.raw_queries,
    }
  }

  pub fn names(&self) -> Vec<&'static str> {
    Capability::ALL
      .iter()
      .filter(|c| self.supports(**c))
      .map(|c| c.as_str())
      .collect()
  }
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
  pub connection: String,
  pub database: Option<String>,
  pub options: HashMap<String, String>,
}

impl ProviderConfig {
  pub fn new(connection: impl Into<String>) -> Self {
    Self {
      connection: connection.into(),
      database: None,
      options: HashMap::new(),
    }
  }

  pub fn with_database(mut self, db: impl Into<String>) -> Self {
    self.database = Some(db.into());
    self
  }

  pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    self.options.insert(key.into(), value.into());
    self
  }
}

#[async_trait]
pub trait Storage: Send + Sync + Clone + 'static {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value>;

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>>;

  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<&str>,
    sort_asc: bool,
  ) -> OrmResult<Vec<Value>>;

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value>;

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value>;

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool>;

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize>;

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize>;

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64>;

  async fn exists(&self, collection: &str, id: &str) -> OrmResult<bool> {
    self
      .find_by_id(collection, id)
      .await
      .map(|opt| opt.is_some())
  }

  async fn find_all(&self, collection: &str) -> OrmResult<Vec<Value>> {
    self
      .find_many(collection, None, None, None, None, true)
      .await
  }

  async fn find_all_typed<T: serde::de::DeserializeOwned + Send>(
    &self,
    collection: &str,
  ) -> OrmResult<Vec<T>> {
    let values = self.find_all(collection).await?;
    values
      .into_iter()
      .map(|v| serde_json::from_value(v).map_err(OrmError::Serialization))
      .collect()
  }

  async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()>;

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()>;

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>>;

  async fn index_exists(&self, collection: &str, index_name: &str) -> OrmResult<bool> {
    let indexes = self.list_indexes(collection).await?;
    Ok(indexes.iter().any(|i| i.name == index_name))
  }

  async fn aggregate(&self, _collection: &str, _pipeline: Vec<Value>) -> OrmResult<Vec<Value>> {
    Err(OrmError::NotSupported(
      "aggregate not implemented".to_string(),
    ))
  }

  async fn health_check(&self) -> OrmResult<bool> {
    Ok(true)
  }

  async fn insert_many(&self, collection: &str, docs: Vec<Value>) -> OrmResult<usize> {
    let mut count = 0;
    for doc in docs {
      self.insert(collection, doc).await?;
      count += 1;
    }
    Ok(count)
  }

  fn capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities::none()
  }

  fn supports(&self, capability: Capability) -> bool {
    self.capabilities().supports(capability)
  }
}

#[async_trait]
pub trait SchemaIntrospection: Send + Sync + Clone + 'static {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>>;
  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema>;
  async fn get_collection_stats(&self, collection: &str) -> OrmResult<CollectionStats>;
  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>>;
  async fn get_database_name(&self) -> OrmResult<String>;
  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    Err(OrmError::NotSupported(
      "list_databases not implemented".to_string(),
    ))
  }
}

#[async_trait]
pub trait AdminCommands: Send + Sync + Clone + 'static {
  async fn execute_raw(&self, sql: &str, params: Vec<Value>) -> OrmResult<RawResult>;
  async fn create_collection(&self, name: &str, schema: Option<CollectionSchema>) -> OrmResult<()>;
  async fn drop_collection(&self, name: &str) -> OrmResult<()>;
  async fn rename_collection(&self, old_name: &str, new_name: &str) -> OrmResult<()>;
  async fn get_server_version(&self) -> OrmResult<String>;
  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth>;
}

#[async_trait]
pub trait StorageBackend: Send + Sync {
  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<&str>,
    sort_asc: bool,
  ) -> OrmResult<Vec<Value>>;

  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value>;

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value>;

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool>;

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64>;

  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>>;

  async fn list_databases(&self) -> OrmResult<Vec<String>>;

  async fn execute_raw(&self, sql: &str, params: Vec<Value>) -> OrmResult<RawResult>;

  async fn health(&self) -> OrmResult<ConnectionHealth>;

  // The methods below are not part of the original narrow backend contract.
  // Their defaults are NotSupported so a backend never claims work it does not
  // do; the blanket impl below forwards each one to the concrete provider.
  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let filter = Filter::Eq("id".to_string(), Value::String(id.to_string()));
    let mut rows = self
      .find_many(collection, Some(&filter), None, Some(1), None, true)
      .await?;
    Ok(rows.pop())
  }

  async fn patch(&self, _collection: &str, _id: &str, _patch: Value) -> OrmResult<Value> {
    Err(OrmError::NotSupported(
      "patch is not supported by this backend".to_string(),
    ))
  }

  async fn update_many(
    &self,
    _collection: &str,
    _filter: Option<Filter>,
    _updates: Value,
  ) -> OrmResult<usize> {
    Err(OrmError::NotSupported(
      "update_many is not supported by this backend".to_string(),
    ))
  }

  async fn delete_many(&self, _collection: &str, _filter: Option<Filter>) -> OrmResult<usize> {
    Err(OrmError::NotSupported(
      "delete_many is not supported by this backend".to_string(),
    ))
  }

  async fn create_index(&self, _collection: &str, _index: &NosqlIndex) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "indexes are not supported by this backend".to_string(),
    ))
  }

  async fn drop_index(&self, _collection: &str, _index_name: &str) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "indexes are not supported by this backend".to_string(),
    ))
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Err(OrmError::NotSupported(
      "indexes are not supported by this backend".to_string(),
    ))
  }

  async fn aggregate(&self, _collection: &str, _pipeline: Vec<Value>) -> OrmResult<Vec<Value>> {
    Err(OrmError::NotSupported(
      "aggregation is not supported by this backend".to_string(),
    ))
  }

  fn capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities::none()
  }

  fn supports(&self, capability: Capability) -> bool {
    self.capabilities().supports(capability)
  }
}

#[async_trait]
impl<P> StorageBackend for P
where
  P: Storage + SchemaIntrospection + AdminCommands + Send + Sync + 'static,
{
  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<&str>,
    sort_asc: bool,
  ) -> OrmResult<Vec<Value>> {
    Storage::find_many(self, collection, filter, skip, limit, sort_by, sort_asc).await
  }

  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    Storage::insert(self, collection, doc).await
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    Storage::update(self, collection, id, doc).await
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    Storage::delete(self, collection, id).await
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    Storage::count(self, collection, filter).await
  }

  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    SchemaIntrospection::list_collections(self).await
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    SchemaIntrospection::list_databases(self).await
  }

  async fn execute_raw(&self, sql: &str, params: Vec<Value>) -> OrmResult<RawResult> {
    AdminCommands::execute_raw(self, sql, params).await
  }

  async fn health(&self) -> OrmResult<ConnectionHealth> {
    AdminCommands::health_check_detailed(self).await
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    Storage::find_by_id(self, collection, id).await
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    Storage::patch(self, collection, id, patch).await
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    Storage::update_many(self, collection, filter, updates).await
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    Storage::delete_many(self, collection, filter).await
  }

  async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()> {
    Storage::create_index(self, collection, index).await
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    Storage::drop_index(self, collection, index_name).await
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Storage::list_indexes(self, collection).await
  }

  async fn aggregate(&self, collection: &str, pipeline: Vec<Value>) -> OrmResult<Vec<Value>> {
    Storage::aggregate(self, collection, pipeline).await
  }

  fn capabilities(&self) -> ProviderCapabilities {
    Storage::capabilities(self)
  }
}

// Bridges the dyn backend returned by connect_backend to the generic
// Repository<E, P> API. Operations outside the backend contract are honest
// NotSupported rather than an invented result.
#[async_trait]
impl Storage for Arc<dyn StorageBackend> {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    StorageBackend::insert(&**self, collection, doc).await
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    StorageBackend::find_by_id(&**self, collection, id).await
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
    StorageBackend::find_many(&**self, collection, filter, skip, limit, sort_by, sort_asc).await
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    StorageBackend::update(&**self, collection, id, doc).await
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    StorageBackend::patch(&**self, collection, id, patch).await
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    StorageBackend::delete(&**self, collection, id).await
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    StorageBackend::delete_many(&**self, collection, filter).await
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    StorageBackend::update_many(&**self, collection, filter, updates).await
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    StorageBackend::count(&**self, collection, filter).await
  }

  async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()> {
    StorageBackend::create_index(&**self, collection, index).await
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    StorageBackend::drop_index(&**self, collection, index_name).await
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    StorageBackend::list_indexes(&**self, collection).await
  }

  async fn aggregate(&self, collection: &str, pipeline: Vec<Value>) -> OrmResult<Vec<Value>> {
    StorageBackend::aggregate(&**self, collection, pipeline).await
  }

  async fn health_check(&self) -> OrmResult<bool> {
    StorageBackend::health(&**self)
      .await
      .map(|health| health.healthy)
  }

  fn capabilities(&self) -> ProviderCapabilities {
    StorageBackend::capabilities(&**self)
  }
}

#[async_trait]
pub trait TransactionControl: Send + Sync + Clone + 'static {
  async fn begin_transaction(&self) -> OrmResult<TransactionId>;
  async fn commit_transaction(&self, id: TransactionId) -> OrmResult<()>;
  async fn rollback_transaction(&self, id: TransactionId) -> OrmResult<()>;
  async fn is_transaction_active(&self) -> OrmResult<bool>;
}

#[cfg(test)]
mod tests;
