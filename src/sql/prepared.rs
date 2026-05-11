use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use tokio::sync::RwLock;

pub struct PreparedStatement {
  pub sql: String,
  pub params: Vec<Value>,
  execution_count: AtomicUsize,
  last_used: AtomicI64,
}

impl PreparedStatement {
  pub fn new(sql: String, params: Vec<Value>) -> Self {
    let now = chrono::Utc::now().timestamp();
    Self {
      sql,
      params,
      execution_count: AtomicUsize::new(0),
      last_used: AtomicI64::new(now),
    }
  }

  pub fn increment_execution(&self) {
    self.execution_count.fetch_add(1, Ordering::Relaxed);
    self
      .last_used
      .store(chrono::Utc::now().timestamp(), Ordering::Relaxed);
  }

  pub fn execution_count(&self) -> usize {
    self.execution_count.load(Ordering::Relaxed)
  }

  pub fn last_used(&self) -> i64 {
    self.last_used.load(Ordering::Relaxed)
  }
}

impl Clone for PreparedStatement {
  fn clone(&self) -> Self {
    Self {
      sql: self.sql.clone(),
      params: self.params.clone(),
      execution_count: AtomicUsize::new(self.execution_count.load(Ordering::Relaxed)),
      last_used: AtomicI64::new(self.last_used.load(Ordering::Relaxed)),
    }
  }
}

impl std::fmt::Debug for PreparedStatement {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("PreparedStatement")
      .field("sql", &self.sql)
      .field("params", &self.params)
      .field(
        "execution_count",
        &self.execution_count.load(Ordering::Relaxed),
      )
      .field("last_used", &self.last_used.load(Ordering::Relaxed))
      .finish()
  }
}

pub struct PreparedStatementCache {
  max_size: usize,
  statements: RwLock<HashMap<String, PreparedStatement>>,
}

impl PreparedStatementCache {
  pub fn new(max_size: usize) -> Self {
    Self {
      max_size,
      statements: RwLock::new(HashMap::new()),
    }
  }

  pub async fn get(&self, sql: &str) -> Option<PreparedStatement> {
    let guard = self.statements.read().await;
    guard.get(sql).cloned()
  }

  pub async fn insert(&self, sql: String, statement: PreparedStatement) {
    let mut guard = self.statements.write().await;
    if guard.len() >= self.max_size {
      if let Some(lru_key) = Self::find_lru_key(&guard) {
        guard.remove(&lru_key);
      }
    }
    guard.insert(sql, statement);
  }

  fn find_lru_key(map: &HashMap<String, PreparedStatement>) -> Option<String> {
    let mut min_last_used = i64::MAX;
    let mut lru_key = None;
    for (key, stmt) in map.iter() {
      let last = stmt.last_used();
      if last < min_last_used {
        min_last_used = last;
        lru_key = Some(key.clone());
      }
    }
    lru_key
  }

  pub async fn remove(&self, sql: &str) -> Option<PreparedStatement> {
    let mut guard = self.statements.write().await;
    guard.remove(sql)
  }

  pub async fn clear(&self) {
    let mut guard = self.statements.write().await;
    guard.clear();
  }

  pub async fn len(&self) -> usize {
    let guard = self.statements.read().await;
    guard.len()
  }

  pub async fn is_empty(&self) -> bool {
    self.len().await == 0
  }

  pub fn max_size(&self) -> usize {
    self.max_size
  }
}

pub struct BatchPreparedStatements {
  cache: PreparedStatementCache,
  batch_size: usize,
}

impl BatchPreparedStatements {
  pub fn new(max_size: usize, batch_size: usize) -> Self {
    Self {
      cache: PreparedStatementCache::new(max_size),
      batch_size,
    }
  }

  pub async fn prepare(
    &self,
    sql: &str,
    params: Vec<Value>,
  ) -> crate::error::OrmResult<PreparedStatement> {
    if let Some(existing) = self.cache.get(sql).await {
      return Ok(existing);
    }
    let stmt = PreparedStatement::new(sql.to_string(), params);
    self.cache.insert(sql.to_string(), stmt.clone()).await;
    Ok(stmt)
  }

  pub async fn get_cached(&self, sql: &str) -> Option<PreparedStatement> {
    self.cache.get(sql).await
  }

  pub async fn invalidate(&self, sql: &str) {
    self.cache.remove(sql).await;
  }

  pub async fn clear(&self) {
    self.cache.clear().await;
  }
}
