use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
pub struct DbQueryLogger<P: DatabaseProvider> {
  inner: Arc<P>,
  collection_name: String,
  enabled: Arc<RwLock<bool>>,
  max_logs: usize,
  retention_count: usize,
}

impl<P: DatabaseProvider + Clone> DbQueryLogger<P> {
  pub fn new(inner: Arc<P>) -> Self {
    Self {
      inner,
      collection_name: "query_logs".to_string(),
      enabled: Arc::new(RwLock::new(true)),
      max_logs: 10000,
      retention_count: 1000,
    }
  }

  pub fn with_collection_name(mut self, name: &str) -> Self {
    self.collection_name = name.to_string();
    self
  }

  pub fn with_max_logs(mut self, max: usize) -> Self {
    self.max_logs = max;
    self
  }

  pub fn with_retention_count(mut self, count: usize) -> Self {
    self.retention_count = count;
    self
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

  async fn insert_log(&self, log_entry: Value) -> OrmResult<()> {
    if !self.is_enabled().await {
      eprintln!("[DbQueryLogger] insert_log skipped - logging disabled");
      return Ok(());
    }
    eprintln!(
      "[DbQueryLogger] insert_log to collection '{}': {:?}",
      self.collection_name, log_entry
    );
    let result = self.inner.insert(&self.collection_name, log_entry).await;
    match &result {
      Ok(_) => eprintln!("[DbQueryLogger] log inserted successfully"),
      Err(e) => eprintln!("[DbQueryLogger] log insert failed: {}", e),
    }
    if let Err(e) = self.trim_logs_if_needed().await {
      eprintln!("[DbQueryLogger] trim failed: {}", e);
    }
    Ok(())
  }

  async fn trim_logs_if_needed(&self) -> OrmResult<()> {
    let count = self.inner.count(&self.collection_name, None).await?;
    eprintln!(
      "[DbQueryLogger] trim_logs_if_needed: count={} max={}",
      count, self.max_logs
    );
    if count as usize > self.max_logs {
      eprintln!("[DbQueryLogger] TRIMMING logs - deleting all entries");
      let _ = self.inner.delete_many(&self.collection_name, None).await;
      eprintln!("[DbQueryLogger] trim complete");
    }
    Ok(())
  }

  fn build_log_entry(
    &self,
    level: &str,
    operation: &str,
    collection: &str,
    document_id: Option<&str>,
    duration_ms: u64,
    success: bool,
    error: Option<&str>,
    filter_summary: Option<&str>,
    result_count: usize,
  ) -> Value {
    let doc_id = document_id
      .map(|s| Value::String(s.to_string()))
      .unwrap_or(Value::Null);
    let err = error
      .map(|s| Value::String(s.to_string()))
      .unwrap_or(Value::Null);

    let mut entry = serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "timestamp": Utc::now().to_rfc3339(),
        "level": level,
        "operation": operation,
        "collection": collection,
        "document_id": doc_id,
        "duration_ms": duration_ms,
        "success": success,
        "error": err,
        "result_count": result_count as i64
    });

    if let Some(fs) = filter_summary {
      entry["filter_summary"] = Value::String(fs.to_string());
    }

    entry
  }
}

#[async_trait]
impl<P: DatabaseProvider + Clone> DatabaseProvider for DbQueryLogger<P> {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    let start = std::time::Instant::now();
    let id = doc.get("id").and_then(|v| v.as_str()).map(String::from);
    let result = self.inner.insert(collection, doc).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    eprintln!(
      "[DbQueryLogger] INSERT collection={} id={:?} duration={}ms success={}",
      collection,
      id,
      duration_ms,
      result.is_ok()
    );

    match &result {
      Ok(v) => {
        let id_str = v.get("id").and_then(|v| v.as_str());
        let log_entry = self.build_log_entry(
          "INFO",
          "INSERT",
          collection,
          id_str,
          duration_ms,
          true,
          None,
          None,
          1,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "INSERT",
          collection,
          id.as_deref(),
          duration_ms,
          false,
          Some(&e.to_string()),
          None,
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let start = std::time::Instant::now();
    let result = self.inner.find_by_id(collection, id).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(Some(doc)) => {
        let log_entry = self.build_log_entry(
          "DEBUG",
          "FIND_BY_ID",
          collection,
          Some(id),
          duration_ms,
          true,
          None,
          None,
          1,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Ok(None) => {
        let log_entry = self.build_log_entry(
          "WARN",
          "FIND_BY_ID",
          collection,
          Some(id),
          duration_ms,
          true,
          None,
          None,
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "FIND_BY_ID",
          collection,
          Some(id),
          duration_ms,
          false,
          Some(&e.to_string()),
          None,
          0,
        );
        let _ = self.insert_log(log_entry).await;
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
    let start = std::time::Instant::now();
    let filter_summary = filter.map(|f| format!("{:?}", f));
    let result = self
      .inner
      .find_many(collection, filter, skip, limit, sort_by, sort_asc)
      .await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(docs) => {
        let log_entry = self.build_log_entry(
          "DEBUG",
          "FIND_MANY",
          collection,
          None,
          duration_ms,
          true,
          None,
          filter_summary.as_deref(),
          docs.len(),
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "FIND_MANY",
          collection,
          None,
          duration_ms,
          false,
          Some(&e.to_string()),
          filter_summary.as_deref(),
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let start = std::time::Instant::now();
    let result = self.inner.update(collection, id, doc).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(v) => {
        let log_entry = self.build_log_entry(
          "INFO",
          "UPDATE",
          collection,
          Some(id),
          duration_ms,
          true,
          None,
          None,
          1,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "UPDATE",
          collection,
          Some(id),
          duration_ms,
          false,
          Some(&e.to_string()),
          None,
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let start = std::time::Instant::now();
    let result = self.inner.patch(collection, id, patch).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(v) => {
        let log_entry = self.build_log_entry(
          "INFO",
          "PATCH",
          collection,
          Some(id),
          duration_ms,
          true,
          None,
          None,
          1,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "PATCH",
          collection,
          Some(id),
          duration_ms,
          false,
          Some(&e.to_string()),
          None,
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let start = std::time::Instant::now();
    let result = self.inner.delete(collection, id).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(true) => {
        let log_entry = self.build_log_entry(
          "INFO",
          "DELETE",
          collection,
          Some(id),
          duration_ms,
          true,
          None,
          None,
          1,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Ok(false) => {
        let log_entry = self.build_log_entry(
          "WARN",
          "DELETE",
          collection,
          Some(id),
          duration_ms,
          true,
          None,
          None,
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "DELETE",
          collection,
          Some(id),
          duration_ms,
          false,
          Some(&e.to_string()),
          None,
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let start = std::time::Instant::now();
    let filter_summary = filter.as_ref().map(|f| format!("{:?}", f));
    let result = self.inner.delete_many(collection, filter).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(count) => {
        let log_entry = self.build_log_entry(
          "INFO",
          "DELETE_MANY",
          collection,
          None,
          duration_ms,
          true,
          None,
          filter_summary.as_deref(),
          *count,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "DELETE_MANY",
          collection,
          None,
          duration_ms,
          false,
          Some(&e.to_string()),
          filter_summary.as_deref(),
          0,
        );
        let _ = self.insert_log(log_entry).await;
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
    let start = std::time::Instant::now();
    let filter_summary = filter.as_ref().map(|f| format!("{:?}", f));
    let result = self.inner.update_many(collection, filter, updates).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(count) => {
        let log_entry = self.build_log_entry(
          "INFO",
          "UPDATE_MANY",
          collection,
          None,
          duration_ms,
          true,
          None,
          filter_summary.as_deref(),
          *count,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "UPDATE_MANY",
          collection,
          None,
          duration_ms,
          false,
          Some(&e.to_string()),
          filter_summary.as_deref(),
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let start = std::time::Instant::now();
    let filter_summary = filter.map(|f| format!("{:?}", f));
    let result = self.inner.count(collection, filter).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(c) => {
        let log_entry = self.build_log_entry(
          "DEBUG",
          "COUNT",
          collection,
          None,
          duration_ms,
          true,
          None,
          filter_summary.as_deref(),
          *c as usize,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "COUNT",
          collection,
          None,
          duration_ms,
          false,
          Some(&e.to_string()),
          filter_summary.as_deref(),
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }

  async fn exists(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let start = std::time::Instant::now();
    let result = self.inner.exists(collection, id).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(exists) => {
        let log_entry = self.build_log_entry(
          "DEBUG",
          "EXISTS",
          collection,
          Some(id),
          duration_ms,
          true,
          None,
          None,
          if *exists { 1 } else { 0 },
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "EXISTS",
          collection,
          Some(id),
          duration_ms,
          false,
          Some(&e.to_string()),
          None,
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }

  async fn find_all(&self, collection: &str) -> OrmResult<Vec<Value>> {
    let start = std::time::Instant::now();
    let result = self.inner.find_all(collection).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(docs) => {
        let log_entry = self.build_log_entry(
          "DEBUG",
          "FIND_ALL",
          collection,
          None,
          duration_ms,
          true,
          None,
          None,
          docs.len(),
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "FIND_ALL",
          collection,
          None,
          duration_ms,
          false,
          Some(&e.to_string()),
          None,
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }

  async fn create_index(
    &self,
    collection: &str,
    index: &crate::nosql_index::NosqlIndex,
  ) -> OrmResult<()> {
    let start = std::time::Instant::now();
    let index_name = index.get_name();
    let result = self.inner.create_index(collection, index).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        let log_entry = self.build_log_entry(
          "INFO",
          "CREATE_INDEX",
          collection,
          None,
          duration_ms,
          true,
          None,
          index_name.map(|n| format!("index_name={}", n)).as_deref(),
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "CREATE_INDEX",
          collection,
          None,
          duration_ms,
          false,
          Some(&e.to_string()),
          index_name.map(|n| format!("index_name={}", n)).as_deref(),
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    let start = std::time::Instant::now();
    let result = self.inner.drop_index(collection, index_name).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(_) => {
        let log_entry = self.build_log_entry(
          "INFO",
          "DROP_INDEX",
          collection,
          None,
          duration_ms,
          true,
          None,
          Some(&format!("index_name={}", index_name)),
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "DROP_INDEX",
          collection,
          None,
          duration_ms,
          false,
          Some(&e.to_string()),
          Some(&format!("index_name={}", index_name)),
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }

  async fn list_indexes(
    &self,
    collection: &str,
  ) -> OrmResult<Vec<crate::nosql_index::NosqlIndexInfo>> {
    let start = std::time::Instant::now();
    let result = self.inner.list_indexes(collection).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match &result {
      Ok(indexes) => {
        let log_entry = self.build_log_entry(
          "DEBUG",
          "LIST_INDEXES",
          collection,
          None,
          duration_ms,
          true,
          None,
          None,
          indexes.len(),
        );
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(
          "ERROR",
          "LIST_INDEXES",
          collection,
          None,
          duration_ms,
          false,
          Some(&e.to_string()),
          None,
          0,
        );
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }
}
