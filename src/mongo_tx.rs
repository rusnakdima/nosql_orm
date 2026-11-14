// MongoDB transactions using a write buffer (sessions require a Client
// reference; the provider holds a Database, so we buffer writes in memory
// and apply on commit, similar to RedisTxStorage).

use crate::error::OrmResult;
use crate::provider::Storage;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
enum PendingOp {
  Insert(String, Value),
  Update(String, String, Value),
  Delete(String, String),
}

#[derive(Clone)]
pub struct MongoTxStorage<P: Storage> {
  inner: P,
  tx_state: Arc<Mutex<Option<Vec<PendingOp>>>>,
}

impl<P: Storage> MongoTxStorage<P> {
  pub fn new(inner: P) -> Self {
    Self {
      inner,
      tx_state: Arc::new(Mutex::new(None)),
    }
  }

  pub fn is_transaction_active(&self) -> bool {
    self.tx_state.lock().expect("lock poisoned").is_some()
  }

  pub async fn begin_transaction(&self) -> OrmResult<()> {
    let mut state = self.tx_state.lock().expect("lock poisoned");
    if state.is_some() {
      return Err(crate::error::OrmError::Transaction(
        "A transaction is already active".to_string(),
      ));
    }
    *state = Some(Vec::new());
    Ok(())
  }

  pub async fn commit_transaction(&self) -> OrmResult<()> {
    let ops = {
      let mut state = self.tx_state.lock().expect("lock poisoned");
      state.take().ok_or_else(|| {
        crate::error::OrmError::Transaction("No active transaction".to_string())
      })?
    };
    for op in ops {
      match op {
        PendingOp::Insert(collection, doc) => {
          self.inner.insert(&collection, doc).await?;
        }
        PendingOp::Update(collection, id, doc) => {
          self.inner.update(&collection, &id, doc).await?;
        }
        PendingOp::Delete(collection, id) => {
          self.inner.delete(&collection, &id).await?;
        }
      }
    }
    Ok(())
  }

  pub async fn rollback_transaction(&self) -> OrmResult<()> {
    let mut state = self.tx_state.lock().expect("lock poisoned");
    if state.is_none() {
      return Err(crate::error::OrmError::Transaction(
        "No active transaction".to_string(),
      ));
    }
    *state = None;
    Ok(())
  }
}

#[async_trait]
impl<P: Storage> Storage for MongoTxStorage<P> {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    {
      let mut state = self.tx_state.lock().expect("lock poisoned");
      if let Some(ops) = state.as_mut() {
        ops.push(PendingOp::Insert(collection.to_string(), doc.clone()));
        return Ok(doc);
      }
    }
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
    {
      let mut state = self.tx_state.lock().expect("lock poisoned");
      if let Some(ops) = state.as_mut() {
        ops.push(PendingOp::Update(collection.to_string(), id.to_string(), doc.clone()));
        return Ok(doc);
      }
    }
    self.inner.update(collection, id, doc).await
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    self.inner.patch(collection, id, patch).await
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    {
      let mut state = self.tx_state.lock().expect("lock poisoned");
      if let Some(ops) = state.as_mut() {
        ops.push(PendingOp::Delete(collection.to_string(), id.to_string()));
        return Ok(true);
      }
    }
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