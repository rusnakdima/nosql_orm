//! A concrete enum wrapper that implements `Storage` for both JsonProvider and
//! MongoProvider, enabling runtime backend switching without trait objects.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::admin_types::IndexInfo;
use crate::error::OrmResult;
use crate::nosql_index::NosqlIndex;
use crate::prelude::Storage;
use crate::providers::json::JsonProvider;

#[cfg(feature = "mongo")]
use crate::providers::mongo::MongoProvider;

#[derive(Clone)]
pub enum StorageVariant {
  Json(Arc<JsonProvider>),
  #[cfg(feature = "mongo")]
  Mongo(Arc<MongoProvider>),
}

impl StorageVariant {
  pub fn json(provider: Arc<JsonProvider>) -> Self {
    Self::Json(provider)
  }

  #[cfg(feature = "mongo")]
  pub fn mongo(provider: Arc<MongoProvider>) -> Self {
    Self::Mongo(provider)
  }

  /// Return the name of the active backend.
  pub fn backend_name(&self) -> &'static str {
    match self {
      Self::Json(_) => "json",
      #[cfg(feature = "mongo")]
      Self::Mongo(_) => "mongodb",
    }
  }
}

#[async_trait]
impl Storage for StorageVariant {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    match self {
      Self::Json(p) => p.insert(collection, doc).await,
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => p.insert(collection, doc).await,
    }
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    match self {
      Self::Json(p) => p.find_by_id(collection, id).await,
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => p.find_by_id(collection, id).await,
    }
  }

  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&crate::prelude::Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<&str>,
    sort_asc: bool,
  ) -> OrmResult<Vec<Value>> {
    match self {
      Self::Json(p) => {
        p.find_many(collection, filter, skip, limit, sort_by, sort_asc)
          .await
      }
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => {
        p.find_many(collection, filter, skip, limit, sort_by, sort_asc)
          .await
      }
    }
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    match self {
      Self::Json(p) => p.update(collection, id, doc).await,
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => p.update(collection, id, doc).await,
    }
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    match self {
      Self::Json(p) => p.patch(collection, id, patch).await,
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => p.patch(collection, id, patch).await,
    }
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    match self {
      Self::Json(p) => p.delete(collection, id).await,
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => p.delete(collection, id).await,
    }
  }

  async fn delete_many(
    &self,
    collection: &str,
    filter: Option<crate::prelude::Filter>,
  ) -> OrmResult<usize> {
    match self {
      Self::Json(p) => p.delete_many(collection, filter).await,
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => p.delete_many(collection, filter).await,
    }
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<crate::prelude::Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    match self {
      Self::Json(p) => p.update_many(collection, filter, updates).await,
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => p.update_many(collection, filter, updates).await,
    }
  }

  async fn count(
    &self,
    collection: &str,
    filter: Option<&crate::prelude::Filter>,
  ) -> OrmResult<u64> {
    match self {
      Self::Json(p) => p.count(collection, filter).await,
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => p.count(collection, filter).await,
    }
  }

  async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()> {
    match self {
      Self::Json(p) => p.create_index(collection, index).await,
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => p.create_index(collection, index).await,
    }
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    match self {
      Self::Json(p) => p.drop_index(collection, index_name).await,
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => p.drop_index(collection, index_name).await,
    }
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    match self {
      Self::Json(p) => p.list_indexes(collection).await,
      #[cfg(feature = "mongo")]
      Self::Mongo(p) => p.list_indexes(collection).await,
    }
  }
}
