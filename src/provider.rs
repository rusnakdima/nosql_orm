use crate::error::{OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::query::Filter;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

pub use crate::admin_types::{
  CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth, FieldInfo, IndexInfo,
  PoolStats, RawResult, TransactionId,
};

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
pub trait DatabaseProvider: Send + Sync + Clone + 'static {
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
    Err(OrmError::Provider("Not implemented".to_string()))
  }

  async fn health_check(&self) -> OrmResult<bool> {
    Err(OrmError::Provider("Not implemented".to_string()))
  }

  async fn insert_many(&self, _collection: &str, _docs: Vec<Value>) -> OrmResult<usize> {
    Err(OrmError::Provider("Not implemented".to_string()))
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
    Err(OrmError::NotSupported("list_databases not implemented".to_string()))
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
pub trait TransactionControl: Send + Sync + Clone + 'static {
  async fn begin_transaction(&self) -> OrmResult<TransactionId>;
  async fn commit_transaction(&self, id: TransactionId) -> OrmResult<()>;
  async fn rollback_transaction(&self, id: TransactionId) -> OrmResult<()>;
  async fn is_transaction_active(&self) -> OrmResult<bool>;
}
