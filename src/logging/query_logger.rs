use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use log::{debug, info, warn};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct QueryLogger<P> {
  inner: P,
  enabled: Arc<RwLock<bool>>,
}

impl<P> QueryLogger<P> {
  pub fn new(inner: P) -> Self {
    Self {
      inner,
      enabled: Arc::new(RwLock::new(true)),
    }
  }

  pub async fn enable(&self) {
    *self.enabled.write().await = true;
  }

  pub async fn disable(&self) {
    *self.enabled.write().await = false;
  }

  pub async fn is_enabled(&self) -> bool {
    *self.enabled.read().await
  }

  fn log_query(&self, operation: &str, collection: &str, filter: Option<&Filter>) {
    let filter_str = filter
      .map(|f| format!(" filter={:?}", f))
      .unwrap_or_default();
    debug!("[Query] {} on '{}'{}", operation, collection, filter_str);
  }

  fn log_result(&self, operation: &str, collection: &str, count: usize) {
    info!(
      "[Query] {} on '{}' returned {} results",
      operation, collection, count
    );
  }
}

#[async_trait::async_trait]
impl<P: DatabaseProvider> DatabaseProvider for QueryLogger<P> {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    debug!("[Query] INSERT '{}'", collection);
    let result = self.inner.insert(collection, doc).await;
    match &result {
      Ok(v) => info!("[Query] INSERT '{}' -> id={:?}", collection, v.get("id")),
      Err(e) => warn!("[Query] INSERT '{}' FAILED: {}", collection, e),
    }
    result
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    debug!("[Query] FIND_BY_ID '{}' id={}", collection, id);
    let result = self.inner.find_by_id(collection, id).await;
    match &result {
      Ok(Some(_v)) => info!("[Query] FIND_BY_ID '{}' id={} -> found", collection, id),
      Ok(None) => info!("[Query] FIND_BY_ID '{}' id={} -> not found", collection, id),
      Err(e) => warn!(
        "[Query] FIND_BY_ID '{}' id={} FAILED: {}",
        collection, id, e
      ),
    }
    result
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
    self.log_query("FIND_MANY", collection, filter);
    let result = self
      .inner
      .find_many(collection, filter, skip, limit, sort_by, sort_asc)
      .await;
    if let Ok(ref docs) = result {
      self.log_result("FIND_MANY", collection, docs.len());
    }
    result
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    debug!("[Query] UPDATE '{}' id={}", collection, id);
    let result = self.inner.update(collection, id, doc).await;
    match &result {
      Ok(_v) => info!("[Query] UPDATE '{}' id={} -> success", collection, id),
      Err(e) => warn!("[Query] UPDATE '{}' id={} FAILED: {}", collection, id, e),
    }
    result
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    debug!("[Query] PATCH '{}' id={}", collection, id);
    let result = self.inner.patch(collection, id, patch).await;
    match &result {
      Ok(_v) => info!("[Query] PATCH '{}' id={} -> success", collection, id),
      Err(e) => warn!("[Query] PATCH '{}' id={} FAILED: {}", collection, id, e),
    }
    result
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    debug!("[Query] DELETE '{}' id={}", collection, id);
    let result = self.inner.delete(collection, id).await;
    match &result {
      Ok(true) => info!("[Query] DELETE '{}' id={} -> deleted", collection, id),
      Ok(false) => info!("[Query] DELETE '{}' id={} -> not found", collection, id),
      Err(e) => warn!("[Query] DELETE '{}' id={} FAILED: {}", collection, id, e),
    }
    result
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    self.log_query("DELETE_MANY", collection, filter.as_ref());
    let result = self.inner.delete_many(collection, filter).await;
    if let Ok(ref count) = result {
      info!("[Query] DELETE_MANY '{}' -> {} deleted", collection, count);
    }
    result
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    self.log_query("UPDATE_MANY", collection, filter.as_ref());
    let result = self.inner.update_many(collection, filter, updates).await;
    if let Ok(ref count) = result {
      info!("[Query] UPDATE_MANY '{}' -> {} updated", collection, count);
    }
    result
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    self.log_query("COUNT", collection, filter);
    let result = self.inner.count(collection, filter).await;
    if let Ok(ref c) = result {
      debug!("[Query] COUNT '{}' -> {}", collection, c);
    }
    result
  }

  async fn exists(&self, collection: &str, id: &str) -> OrmResult<bool> {
    debug!("[Query] EXISTS '{}' id={}", collection, id);
    self.inner.exists(collection, id).await
  }

  async fn find_all(&self, collection: &str) -> OrmResult<Vec<Value>> {
    debug!("[Query] FIND_ALL '{}'", collection);
    let result = self.inner.find_all(collection).await;
    if let Ok(ref docs) = result {
      info!(
        "[Query] FIND_ALL '{}' -> {} results",
        collection,
        docs.len()
      );
    }
    result
  }

  async fn create_index(
    &self,
    collection: &str,
    index: &crate::nosql_index::NosqlIndex,
  ) -> OrmResult<()> {
    info!(
      "[Query] CREATE_INDEX '{}' name={:?}",
      collection,
      index.get_name()
    );
    self.inner.create_index(collection, index).await
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    info!("[Query] DROP_INDEX '{}' name={}", collection, index_name);
    self.inner.drop_index(collection, index_name).await
  }

  async fn list_indexes(
    &self,
    collection: &str,
  ) -> OrmResult<Vec<crate::nosql_index::NosqlIndexInfo>> {
    debug!("[Query] LIST_INDEXES '{}'", collection);
    self.inner.list_indexes(collection).await
  }
}
