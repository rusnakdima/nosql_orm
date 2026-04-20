use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct FileQueryLogger<P: DatabaseProvider> {
  inner: Arc<P>,
  log_path: PathBuf,
  enabled: Arc<RwLock<bool>>,
}

impl<P: DatabaseProvider> FileQueryLogger<P> {
  pub fn new(inner: Arc<P>, log_dir: PathBuf) -> Self {
    std::fs::create_dir_all(&log_dir).ok();
    Self {
      inner,
      log_path: log_dir.join("query_log.txt"),
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

  fn write_log(&self, entry: &str) {
    if let Ok(mut file) = OpenOptions::new()
      .create(true)
      .append(true)
      .open(&self.log_path)
    {
      let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
      let _ = writeln!(file, "[{}] {}", timestamp, entry);
    }
  }

  fn log_query(&self, operation: &str, collection: &str, filter: Option<&Filter>) {
    let filter_str = filter
      .map(|f| format!(" filter={:?}", f))
      .unwrap_or_default();
    let entry = format!("[Query] {} on '{}'{}", operation, collection, filter_str);
    self.write_log(&entry);
  }

  fn log_result(&self, operation: &str, collection: &str, count: usize) {
    let entry = format!(
      "[Query] {} on '{}' returned {} results",
      operation, collection, count
    );
    self.write_log(&entry);
  }

  fn log_error(&self, operation: &str, collection: &str, error: &str) {
    let entry = format!(
      "[Query] {} on '{}' FAILED: {}",
      operation, collection, error
    );
    self.write_log(&entry);
  }

  fn log_success(&self, operation: &str, collection: &str, id: Option<&str>) {
    let id_str = id.map(|s| format!(" id={}", s)).unwrap_or_default();
    let entry = format!(
      "[Query] {} on '{}' -> success{}",
      operation, collection, id_str
    );
    self.write_log(&entry);
  }
}

impl<P: DatabaseProvider + Clone> FileQueryLogger<P> {
  pub async fn get_recent_logs(&self, count: usize) -> Vec<String> {
    let content = std::fs::read_to_string(&self.log_path).unwrap_or_default();
    content
      .lines()
      .rev()
      .take(count)
      .map(|s| s.to_string())
      .collect()
  }

  pub fn get_log_path(&self) -> &PathBuf {
    &self.log_path
  }

  pub fn clear_logs(&self) -> std::io::Result<()> {
    std::fs::write(&self.log_path, "")
  }
}

#[async_trait::async_trait]
impl<P: DatabaseProvider + Clone> DatabaseProvider for FileQueryLogger<P> {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    let id = doc.get("id").and_then(|v| v.as_str());
    self.write_log(&format!("[Query] INSERT '{}' id={:?}", collection, id));
    let result = self.inner.insert(collection, doc).await;
    match &result {
      Ok(v) => {
        let id = v.get("id").and_then(|v| v.as_str());
        self.write_log(&format!("[Query] INSERT '{}' -> id={:?}", collection, id));
      }
      Err(e) => {
        self.log_error("INSERT", collection, &e.to_string());
      }
    }
    result
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    self.write_log(&format!("[Query] FIND_BY_ID '{}' id={}", collection, id));
    let result = self.inner.find_by_id(collection, id).await;
    match &result {
      Ok(Some(_)) => {
        self.write_log(&format!(
          "[Query] FIND_BY_ID '{}' id={} -> found",
          collection, id
        ));
      }
      Ok(None) => {
        self.write_log(&format!(
          "[Query] FIND_BY_ID '{}' id={} -> not found",
          collection, id
        ));
      }
      Err(e) => {
        self.log_error("FIND_BY_ID", collection, &e.to_string());
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
    self.write_log(&format!("[Query] UPDATE '{}' id={}", collection, id));
    let result = self.inner.update(collection, id, doc).await;
    match &result {
      Ok(_) => {
        self.log_success("UPDATE", collection, Some(id));
      }
      Err(e) => {
        self.log_error("UPDATE", collection, &e.to_string());
      }
    }
    result
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    self.write_log(&format!("[Query] PATCH '{}' id={}", collection, id));
    let result = self.inner.patch(collection, id, patch).await;
    match &result {
      Ok(_) => {
        self.log_success("PATCH", collection, Some(id));
      }
      Err(e) => {
        self.log_error("PATCH", collection, &e.to_string());
      }
    }
    result
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    self.write_log(&format!("[Query] DELETE '{}' id={}", collection, id));
    let result = self.inner.delete(collection, id).await;
    match &result {
      Ok(true) => {
        self.write_log(&format!(
          "[Query] DELETE '{}' id={} -> deleted",
          collection, id
        ));
      }
      Ok(false) => {
        self.write_log(&format!(
          "[Query] DELETE '{}' id={} -> not found",
          collection, id
        ));
      }
      Err(e) => {
        self.log_error("DELETE", collection, &e.to_string());
      }
    }
    result
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    self.log_query("DELETE_MANY", collection, filter.as_ref());
    let result = self.inner.delete_many(collection, filter).await;
    if let Ok(ref count) = result {
      self.write_log(&format!(
        "[Query] DELETE_MANY '{}' -> {} deleted",
        collection, count
      ));
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
      self.write_log(&format!(
        "[Query] UPDATE_MANY '{}' -> {} updated",
        collection, count
      ));
    }
    result
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    self.log_query("COUNT", collection, filter);
    let result = self.inner.count(collection, filter).await;
    if let Ok(ref c) = result {
      self.write_log(&format!("[Query] COUNT '{}' -> {}", collection, c));
    }
    result
  }

  async fn exists(&self, collection: &str, id: &str) -> OrmResult<bool> {
    self.write_log(&format!("[Query] EXISTS '{}' id={}", collection, id));
    self.inner.exists(collection, id).await
  }

  async fn find_all(&self, collection: &str) -> OrmResult<Vec<Value>> {
    self.write_log(&format!("[Query] FIND_ALL '{}'", collection));
    let result = self.inner.find_all(collection).await;
    if let Ok(ref docs) = result {
      self.write_log(&format!(
        "[Query] FIND_ALL '{}' -> {} results",
        collection,
        docs.len()
      ));
    }
    result
  }

  async fn create_index(
    &self,
    collection: &str,
    index: &crate::nosql_index::NosqlIndex,
  ) -> OrmResult<()> {
    self.write_log(&format!(
      "[Query] CREATE_INDEX '{}' name={:?}",
      collection,
      index.get_name()
    ));
    self.inner.create_index(collection, index).await
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    self.write_log(&format!(
      "[Query] DROP_INDEX '{}' name={}",
      collection, index_name
    ));
    self.inner.drop_index(collection, index_name).await
  }

  async fn list_indexes(
    &self,
    collection: &str,
  ) -> OrmResult<Vec<crate::nosql_index::NosqlIndexInfo>> {
    self.write_log(&format!("[Query] LIST_INDEXES '{}'", collection));
    self.inner.list_indexes(collection).await
  }
}
