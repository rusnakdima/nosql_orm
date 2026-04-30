use crate::error::OrmResult;
use crate::logging::LoggingStrategy;
use crate::provider::DatabaseProvider;
use crate::providers::JsonProvider;
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
pub struct DbQueryLogger {
  inner: Arc<JsonProvider>,
  collection_name: String,
  enabled: Arc<RwLock<bool>>,
  max_logs: usize,
  retention_count: usize,
}

impl DbQueryLogger {
  pub fn new(inner: JsonProvider) -> Self {
    Self {
      inner: Arc::new(inner),
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
impl LoggingStrategy for DbQueryLogger {
  async fn log_start(&self, operation: &str, collection: &str) {
    eprintln!(
      "[DbQueryLogger] {} collection={} - starting",
      operation, collection
    );
  }

  async fn log_complete(&self, operation: &str, collection: &str, duration_ms: u64, success: bool) {
    eprintln!(
      "[DbQueryLogger] {} collection={} duration={}ms success={}",
      operation, collection, duration_ms, success
    );

    let level = if success { "INFO" } else { "ERROR" }.to_string();
    let log_entry = self.build_log_entry(LogEntry {
      level,
      operation: operation.to_string(),
      collection: collection.to_string(),
      document_id: None,
      duration_ms,
      success,
      error: None,
      filter_summary: None,
      result_count: 0,
    });
    let _ = self.insert_log(log_entry).await;
  }

  async fn log_error(&self, operation: &str, collection: &str, error: &str) {
    eprintln!(
      "[DbQueryLogger] {} collection={} ERROR: {}",
      operation, collection, error
    );

    let log_entry = self.build_log_entry(LogEntry {
      level: "ERROR".to_string(),
      operation: operation.to_string(),
      collection: collection.to_string(),
      document_id: None,
      duration_ms: 0,
      success: false,
      error: Some(error.to_string()),
      filter_summary: None,
      result_count: 0,
    });
    let _ = self.insert_log(log_entry).await;
  }
}
