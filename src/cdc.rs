// Change Data Capture (CDC): stream row-level changes from a collection.
//
// Wraps any `Storage` and records insert/update/delete events to an
// in-memory change log that can be streamed or polled.

use crate::error::OrmResult;
use crate::provider::Storage;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::{self, Stream};
use serde_json::Value;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum ChangeOp {
  Insert,
  Update,
  Delete,
}

#[derive(Debug, Clone)]
pub struct ChangeEvent {
  pub op: ChangeOp,
  pub collection: String,
  pub id: String,
  pub before: Option<Value>,
  pub after: Option<Value>,
  pub timestamp: DateTime<Utc>,
}

impl ChangeEvent {
  pub fn insert(collection: &str, doc: Value) -> Self {
    let id = doc
      .get("id")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_string();
    Self {
      op: ChangeOp::Insert,
      collection: collection.to_string(),
      id,
      before: None,
      after: Some(doc),
      timestamp: Utc::now(),
    }
  }

  pub fn update(collection: &str, before: Value, after: Value) -> Self {
    let id = after
      .get("id")
      .or_else(|| before.get("id"))
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_string();
    Self {
      op: ChangeOp::Update,
      collection: collection.to_string(),
      id,
      before: Some(before),
      after: Some(after),
      timestamp: Utc::now(),
    }
  }

  pub fn delete(collection: &str, doc: Value) -> Self {
    let id = doc
      .get("id")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_string();
    Self {
      op: ChangeOp::Delete,
      collection: collection.to_string(),
      id,
      before: Some(doc),
      after: None,
      timestamp: Utc::now(),
    }
  }
}

#[derive(Clone)]
pub struct ChangeCaptureStorage<P: Storage> {
  inner: P,
  log: Arc<Mutex<VecDeque<ChangeEvent>>>,
  max_log_size: usize,
}

impl<P: Storage> ChangeCaptureStorage<P> {
  pub fn new(inner: P) -> Self {
    Self {
      inner,
      log: Arc::new(Mutex::new(VecDeque::new())),
      max_log_size: 10000,
    }
  }

  pub fn with_max_log_size(mut self, size: usize) -> Self {
    self.max_log_size = size;
    self
  }

  // Drain all pending change events.
  pub fn drain_changes(&self) -> Vec<ChangeEvent> {
    let mut log = self.log.lock().expect("lock poisoned");
    log.drain(..).collect()
  }

  // Peek at pending change events without removing them.
  pub fn peek_changes(&self) -> Vec<ChangeEvent> {
    let log = self.log.lock().expect("lock poisoned");
    log.iter().cloned().collect()
  }

  // Stream change events (drains the log).
  pub fn changes_stream(&self) -> Pin<Box<dyn Stream<Item = ChangeEvent> + Send>> {
    let events = self.drain_changes();
    Box::pin(stream::iter(events))
  }

  fn record(&self, event: ChangeEvent) {
    let mut log = self.log.lock().expect("lock poisoned");
    if log.len() >= self.max_log_size {
      log.pop_front();
    }
    log.push_back(event);
  }
}

#[async_trait]
impl<P: Storage> Storage for ChangeCaptureStorage<P> {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    let result = self.inner.insert(collection, doc.clone()).await?;
    self.record(ChangeEvent::insert(collection, result.clone()));
    Ok(result)
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
    let before = self.inner.find_by_id(collection, id).await?;
    let result = self.inner.update(collection, id, doc).await?;
    if let Some(before) = before {
      self.record(ChangeEvent::update(collection, before, result.clone()));
    }
    Ok(result)
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let before = self.inner.find_by_id(collection, id).await?;
    let result = self.inner.patch(collection, id, patch).await?;
    if let Some(before) = before {
      self.record(ChangeEvent::update(collection, before, result.clone()));
    }
    Ok(result)
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let before = self.inner.find_by_id(collection, id).await?;
    let result = self.inner.delete(collection, id).await?;
    if result {
      if let Some(doc) = before {
        self.record(ChangeEvent::delete(collection, doc));
      }
    }
    Ok(result)
  }

  async fn delete_many(&self, collection: &str, filter: Option<crate::query::Filter>) -> OrmResult<usize> {
    let docs = self
      .inner
      .find_many(collection, filter.as_ref(), None, None, None, true)
      .await?;
    let count = self.inner.delete_many(collection, filter).await?;
    for doc in docs {
      self.record(ChangeEvent::delete(collection, doc));
    }
    Ok(count)
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
    let count = self.inner.insert_many(collection, docs.clone()).await?;
    for doc in docs {
      self.record(ChangeEvent::insert(collection, doc));
    }
    Ok(count)
  }
}