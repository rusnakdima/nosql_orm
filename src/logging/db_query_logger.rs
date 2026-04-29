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
struct LogEntry {
  level: String,
  operation: String,
  collection: String,
  document_id: Option<String>,
  duration_ms: u64,
  success: bool,
  error: Option<String>,
  filter_summary: Option<String>,
  result_count: usize,
}

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

  fn build_log_entry(&self, entry: LogEntry) -> Value {
    let doc_id = entry.document_id.map(Value::String).unwrap_or(Value::Null);
    let err = entry.error.map(Value::String).unwrap_or(Value::Null);

    let mut value = serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "timestamp": Utc::now().to_rfc3339(),
        "level": entry.level,
        "operation": entry.operation,
        "collection": entry.collection,
        "document_id": doc_id,
        "duration_ms": entry.duration_ms,
        "success": entry.success,
        "error": err,
        "result_count": entry.result_count as i64
    });

    if let Some(fs) = entry.filter_summary {
      value["filter_summary"] = Value::String(fs);
    }

    value
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
        let log_entry = self.build_log_entry(LogEntry {
          level: "INFO".to_string(),
          operation: "INSERT".to_string(),
          collection: collection.to_string(),
          document_id: id_str.map(String::from),
          duration_ms,
          success: true,
          error: None,
          filter_summary: None,
          result_count: 1,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "INSERT".to_string(),
          collection: collection.to_string(),
          document_id: id.as_deref().map(String::from),
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: None,
          result_count: 0,
        });
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
      Ok(Some(_doc)) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "DEBUG".to_string(),
          operation: "FIND_BY_ID".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: true,
          error: None,
          filter_summary: None,
          result_count: 1,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Ok(None) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "WARN".to_string(),
          operation: "FIND_BY_ID".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: true,
          error: None,
          filter_summary: None,
          result_count: 0,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "FIND_BY_ID".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: None,
          result_count: 0,
        });
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
        let log_entry = self.build_log_entry(LogEntry {
          level: "DEBUG".to_string(),
          operation: "FIND_MANY".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: true,
          error: None,
          filter_summary: filter_summary.clone(),
          result_count: docs.len(),
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "FIND_MANY".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: filter_summary.clone(),
          result_count: 0,
        });
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
      Ok(_v) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "INFO".to_string(),
          operation: "UPDATE".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: true,
          error: None,
          filter_summary: None,
          result_count: 1,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "UPDATE".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: None,
          result_count: 0,
        });
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
      Ok(_v) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "INFO".to_string(),
          operation: "PATCH".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: true,
          error: None,
          filter_summary: None,
          result_count: 1,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "PATCH".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: None,
          result_count: 0,
        });
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
        let log_entry = self.build_log_entry(LogEntry {
          level: "INFO".to_string(),
          operation: "DELETE".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: true,
          error: None,
          filter_summary: None,
          result_count: 1,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Ok(false) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "WARN".to_string(),
          operation: "DELETE".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: true,
          error: None,
          filter_summary: None,
          result_count: 0,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "DELETE".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: None,
          result_count: 0,
        });
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
        let log_entry = self.build_log_entry(LogEntry {
          level: "INFO".to_string(),
          operation: "DELETE_MANY".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: true,
          error: None,
          filter_summary: filter_summary.clone(),
          result_count: *count,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "DELETE_MANY".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: filter_summary.clone(),
          result_count: 0,
        });
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
        let log_entry = self.build_log_entry(LogEntry {
          level: "INFO".to_string(),
          operation: "UPDATE_MANY".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: true,
          error: None,
          filter_summary: filter_summary.clone(),
          result_count: *count,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "UPDATE_MANY".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: filter_summary.clone(),
          result_count: 0,
        });
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
        let log_entry = self.build_log_entry(LogEntry {
          level: "DEBUG".to_string(),
          operation: "COUNT".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: true,
          error: None,
          filter_summary: filter_summary.clone(),
          result_count: *c as usize,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "COUNT".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: filter_summary.clone(),
          result_count: 0,
        });
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
        let log_entry = self.build_log_entry(LogEntry {
          level: "DEBUG".to_string(),
          operation: "EXISTS".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: true,
          error: None,
          filter_summary: None,
          result_count: if *exists { 1 } else { 0 },
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "EXISTS".to_string(),
          collection: collection.to_string(),
          document_id: Some(id.to_string()),
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: None,
          result_count: 0,
        });
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
        let log_entry = self.build_log_entry(LogEntry {
          level: "DEBUG".to_string(),
          operation: "FIND_ALL".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: true,
          error: None,
          filter_summary: None,
          result_count: docs.len(),
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "FIND_ALL".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: None,
          result_count: 0,
        });
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
        let log_entry = self.build_log_entry(LogEntry {
          level: "INFO".to_string(),
          operation: "CREATE_INDEX".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: true,
          error: None,
          filter_summary: index_name.map(|n| format!("index_name={}", n)),
          result_count: 0,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "CREATE_INDEX".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: index_name.map(|n| format!("index_name={}", n)),
          result_count: 0,
        });
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
        let log_entry = self.build_log_entry(LogEntry {
          level: "INFO".to_string(),
          operation: "DROP_INDEX".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: true,
          error: None,
          filter_summary: Some(format!("index_name={}", index_name)),
          result_count: 0,
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "DROP_INDEX".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: Some(format!("index_name={}", index_name)),
          result_count: 0,
        });
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
        let log_entry = self.build_log_entry(LogEntry {
          level: "DEBUG".to_string(),
          operation: "LIST_INDEXES".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: true,
          error: None,
          filter_summary: None,
          result_count: indexes.len(),
        });
        let _ = self.insert_log(log_entry).await;
      }
      Err(e) => {
        let log_entry = self.build_log_entry(LogEntry {
          level: "ERROR".to_string(),
          operation: "LIST_INDEXES".to_string(),
          collection: collection.to_string(),
          document_id: None,
          duration_ms,
          success: false,
          error: Some(e.to_string()),
          filter_summary: None,
          result_count: 0,
        });
        let _ = self.insert_log(log_entry).await;
      }
    }

    result
  }
}
