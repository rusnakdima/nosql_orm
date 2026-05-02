use crate::error::OrmResult;
use crate::nosql_index::NosqlIndex;
use crate::provider::{DatabaseProvider, IndexInfo};
use crate::query::Filter;
use async_trait::async_trait;
use serde_json::Value;
use std::time::Instant;

#[async_trait]
pub trait LoggingStrategy: Send + Sync + Clone + 'static {
  async fn log_start(&self, operation: &str, collection: &str);
  async fn log_complete(&self, operation: &str, collection: &str, duration_ms: u64, success: bool);
  async fn log_error(&self, operation: &str, collection: &str, error: &str);
}

#[derive(Clone)]
pub struct ProviderWrapper<P, L> {
  inner: P,
  logger: L,
}

impl<P, L> ProviderWrapper<P, L> {
  pub fn new(inner: P, logger: L) -> Self {
    Self { inner, logger }
  }
}

#[async_trait]
impl<P: DatabaseProvider, L: LoggingStrategy + Clone + 'static> DatabaseProvider
  for ProviderWrapper<P, L>
{
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    let start = Instant::now();
    self.logger.log_start("INSERT", collection).await;
    let result = self.inner.insert(collection, doc).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("INSERT", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("INSERT", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let start = Instant::now();
    self.logger.log_start("FIND_BY_ID", collection).await;
    let result = self.inner.find_by_id(collection, id).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(Some(_doc)) => {
        self
          .logger
          .log_complete("FIND_BY_ID", collection, duration_ms, true)
          .await;
      }
      Ok(None) => {
        self
          .logger
          .log_complete("FIND_BY_ID", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("FIND_BY_ID", collection, &e.to_string())
          .await;
      }
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
    let start = Instant::now();
    self.logger.log_start("FIND_MANY", collection).await;
    let result = self
      .inner
      .find_many(collection, filter, skip, limit, sort_by, sort_asc)
      .await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("FIND_MANY", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("FIND_MANY", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let start = Instant::now();
    self.logger.log_start("UPDATE", collection).await;
    let result = self.inner.update(collection, id, doc).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("UPDATE", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("UPDATE", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let start = Instant::now();
    self.logger.log_start("PATCH", collection).await;
    let result = self.inner.patch(collection, id, patch).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("PATCH", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("PATCH", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let start = Instant::now();
    self.logger.log_start("DELETE", collection).await;
    let result = self.inner.delete(collection, id).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(true) => {
        self
          .logger
          .log_complete("DELETE", collection, duration_ms, true)
          .await;
      }
      Ok(false) => {
        self
          .logger
          .log_complete("DELETE", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("DELETE", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let start = Instant::now();
    self.logger.log_start("DELETE_MANY", collection).await;
    let result = self.inner.delete_many(collection, filter).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("DELETE_MANY", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("DELETE_MANY", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    let start = Instant::now();
    self.logger.log_start("UPDATE_MANY", collection).await;
    let result = self.inner.update_many(collection, filter, updates).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("UPDATE_MANY", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("UPDATE_MANY", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let start = Instant::now();
    self.logger.log_start("COUNT", collection).await;
    let result = self.inner.count(collection, filter).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("COUNT", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("COUNT", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn exists(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let start = Instant::now();
    self.logger.log_start("EXISTS", collection).await;
    let result = self.inner.exists(collection, id).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("EXISTS", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("EXISTS", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn find_all(&self, collection: &str) -> OrmResult<Vec<Value>> {
    let start = Instant::now();
    self.logger.log_start("FIND_ALL", collection).await;
    let result = self.inner.find_all(collection).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("FIND_ALL", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("FIND_ALL", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn find_all_typed<T: serde::de::DeserializeOwned + Send>(
    &self,
    collection: &str,
  ) -> OrmResult<Vec<T>> {
    let start = Instant::now();
    self.logger.log_start("FIND_ALL_TYPED", collection).await;
    let result = self.inner.find_all_typed(collection).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("FIND_ALL_TYPED", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("FIND_ALL_TYPED", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()> {
    let start = Instant::now();
    self.logger.log_start("CREATE_INDEX", collection).await;
    let result = self.inner.create_index(collection, index).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("CREATE_INDEX", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("CREATE_INDEX", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    let start = Instant::now();
    self.logger.log_start("DROP_INDEX", collection).await;
    let result = self.inner.drop_index(collection, index_name).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("DROP_INDEX", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("DROP_INDEX", collection, &e.to_string())
          .await;
      }
    }

    result
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    let start = Instant::now();
    self.logger.log_start("LIST_INDEXES", collection).await;
    let result = self.inner.list_indexes(collection).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        self
          .logger
          .log_complete("LIST_INDEXES", collection, duration_ms, true)
          .await;
      }
      Err(e) => {
        self
          .logger
          .log_error("LIST_INDEXES", collection, &e.to_string())
          .await;
      }
    }

    result
  }
}
