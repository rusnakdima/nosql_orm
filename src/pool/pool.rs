use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct PoolConfig {
  pub max_size: usize,
  pub min_idle: Option<usize>,
  pub wait_for_available: bool,
  pub idle_timeout_secs: Option<u64>,
}

impl Default for PoolConfig {
  fn default() -> Self {
    Self {
      max_size: 10,
      min_idle: None,
      wait_for_available: true,
      idle_timeout_secs: None,
    }
  }
}

impl PoolConfig {
  pub fn new(max_size: usize) -> Self {
    Self {
      max_size,
      ..Default::default()
    }
  }

  pub fn min_idle(mut self, n: usize) -> Self {
    self.min_idle = Some(n);
    self
  }

  pub fn wait_for_available(mut self, wait: bool) -> Self {
    self.wait_for_available = wait;
    self
  }

  pub fn idle_timeout_secs(mut self, secs: u64) -> Self {
    self.idle_timeout_secs = Some(secs);
    self
  }
}

pub struct Pooled<T> {
  inner: T,
  pool: Option<Arc<PoolInner>>,
}

impl<T> Pooled<T> {
  pub fn new(inner: T) -> Self {
    Self { inner, pool: None }
  }

  pub fn from_pool(inner: T, pool: Arc<PoolInner>) -> Self {
    Self {
      inner,
      pool: Some(pool),
    }
  }

  #[allow(dead_code)]
  pub fn inner(&self) -> &T {
    &self.inner
  }

  pub fn inner_mut(&mut self) -> &mut T {
    &mut self.inner
  }
}

impl<T> std::ops::Deref for Pooled<T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<T> std::ops::DerefMut for Pooled<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

impl<T> Drop for Pooled<T> {
  fn drop(&mut self) {
    if let Some(pool) = self.pool.take() {
      pool.release();
    }
  }
}

pub(crate) struct PoolInner {
  semaphore: Semaphore,
  available: std::sync::atomic::AtomicUsize,
  #[allow(dead_code)]
  total: std::sync::atomic::AtomicUsize,
}

impl PoolInner {
  fn new(max_size: usize) -> Self {
    Self {
      semaphore: Semaphore::new(max_size),
      available: std::sync::atomic::AtomicUsize::new(max_size),
      total: std::sync::atomic::AtomicUsize::new(max_size),
    }
  }

  async fn acquire(&self, wait_for_available: bool) -> OrmResult<()> {
    if wait_for_available {
      let permit = self
        .semaphore
        .acquire()
        .await
        .map_err(|_| OrmError::Connection("Pool acquire failed".to_string()))?;
      self
        .available
        .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
      drop(permit);
      Ok(())
    } else {
      let permit = self
        .semaphore
        .try_acquire()
        .map_err(|_| OrmError::Connection("No available connections in pool".to_string()))?;
      self
        .available
        .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
      drop(permit);
      Ok(())
    }
  }

  fn release(&self) {
    self
      .available
      .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    self.semaphore.add_permits(1);
  }
}

pub struct Pool<P: DatabaseProvider> {
  inner: Arc<PoolInner>,
  _phantom: std::marker::PhantomData<P>,
}

impl<P: DatabaseProvider> Pool<P> {
  pub fn with_config(_config: PoolConfig) -> Self {
    Self {
      inner: Arc::new(PoolInner::new(_config.max_size)),
      _phantom: std::marker::PhantomData,
    }
  }

  pub async fn acquire(&self, wait_for_available: bool) -> OrmResult<()> {
    self.inner.acquire(wait_for_available).await
  }
}

#[derive(Clone)]
pub struct JsonPool {
  base_dir: std::path::PathBuf,
  pool: Arc<PoolInner>,
  cache: Arc<tokio::sync::RwLock<HashMap<String, Vec<Value>>>>,
}

impl JsonPool {
  pub async fn with_config(base_dir: std::path::PathBuf, config: PoolConfig) -> OrmResult<Self> {
    tokio::fs::create_dir_all(&base_dir).await?;
    Ok(Self {
      base_dir,
      pool: Arc::new(PoolInner::new(config.max_size)),
      cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    })
  }

  pub async fn acquire(&self, wait_for_available: bool) -> OrmResult<PooledJson> {
    self.pool.acquire(wait_for_available).await?;
    Ok(PooledJson {
      base_dir: self.base_dir.clone(),
      cache: self.cache.clone(),
      pool: Some(self.pool.clone()),
    })
  }

  #[allow(dead_code)]
  pub fn pool(&self) -> &Arc<PoolInner> {
    &self.pool
  }
}

#[derive(Clone)]
pub struct PooledJson {
  base_dir: std::path::PathBuf,
  cache: Arc<tokio::sync::RwLock<HashMap<String, Vec<Value>>>>,
  pool: Option<Arc<PoolInner>>,
}

impl PooledJson {
  fn collection_path(&self, collection: &str) -> std::path::PathBuf {
    self.base_dir.join(format!("{}.json", collection))
  }

  async fn ensure_loaded(&self, collection: &str) -> OrmResult<()> {
    {
      let r = self.cache.read().await;
      if r.contains_key(collection) {
        return Ok(());
      }
    }

    let path = self.collection_path(collection);
    let records: Vec<Value> = if path.exists() {
      let raw = tokio::fs::read_to_string(&path).await?;
      serde_json::from_str(&raw)?
    } else {
      vec![]
    };

    let mut w = self.cache.write().await;
    w.entry(collection.to_string()).or_insert(records);
    Ok(())
  }

  async fn flush(&self, collection: &str) -> OrmResult<()> {
    let r = self.cache.read().await;
    if let Some(records) = r.get(collection) {
      let path = self.collection_path(collection);
      let json_str = serde_json::to_string_pretty(records)?;
      tokio::fs::write(&path, json_str).await?;
    }
    Ok(())
  }
}

impl Drop for PooledJson {
  fn drop(&mut self) {
    if let Some(pool) = self.pool.take() {
      pool.release();
    }
  }
}

#[async_trait]
impl DatabaseProvider for PooledJson {
  async fn insert(&self, collection: &str, mut doc: Value) -> OrmResult<Value> {
    self.ensure_loaded(collection).await?;

    if doc
      .get("id")
      .and_then(|v| v.as_str())
      .map_or(true, |s| s.is_empty())
    {
      doc["id"] = serde_json::json!(crate::utils::generate_id());
    }

    let mut w = self.cache.write().await;
    let records = w.entry(collection.to_string()).or_default();

    let id = doc["id"].as_str().unwrap().to_string();
    if records
      .iter()
      .any(|r| r.get("id").and_then(|v| v.as_str()) == Some(&id))
    {
      return Err(OrmError::Duplicate(format!("id={}", id)));
    }
    records.push(doc.clone());
    drop(w);

    self.flush(collection).await?;
    Ok(doc)
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    self.ensure_loaded(collection).await?;
    let r = self.cache.read().await;
    Ok(
      r.get(collection)
        .and_then(|recs| {
          recs
            .iter()
            .find(|d| d.get("id").and_then(|v| v.as_str()) == Some(id))
        })
        .cloned(),
    )
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
    self.ensure_loaded(collection).await?;
    let r = self.cache.read().await;
    let records = match r.get(collection) {
      Some(v) => v.clone(),
      None => return Ok(vec![]),
    };
    drop(r);

    let mut results: Vec<Value> = records
      .into_iter()
      .filter(|d| filter.map_or(true, |f| f.matches(d)))
      .collect();

    if let Some(field) = sort_by {
      results.sort_by(|a, b| {
        let av = a.get(field);
        let bv = b.get(field);
        let ord = compare_values(av, bv);
        if sort_asc {
          ord
        } else {
          ord.reverse()
        }
      });
    }

    let skip = skip.unwrap_or(0) as usize;
    let results: Vec<Value> = results.into_iter().skip(skip).collect();
    let results = match limit {
      Some(n) => results.into_iter().take(n as usize).collect(),
      None => results,
    };

    Ok(results)
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    self.ensure_loaded(collection).await?;
    let mut w = self.cache.write().await;
    let records = w
      .get_mut(collection)
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;

    let pos = records
      .iter()
      .position(|r| r.get("id").and_then(|v| v.as_str()) == Some(id))
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;

    records[pos] = doc.clone();
    drop(w);
    self.flush(collection).await?;
    Ok(doc)
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    self.ensure_loaded(collection).await?;
    let mut w = self.cache.write().await;
    let records = w
      .get_mut(collection)
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;

    let pos = records
      .iter()
      .position(|r| r.get("id").and_then(|v| v.as_str()) == Some(id))
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;

    if let (Value::Object(base), Value::Object(updates)) = (&mut records[pos], patch) {
      for (k, v) in updates {
        base.insert(k.clone(), v.clone());
      }
    }
    let updated = records[pos].clone();
    drop(w);
    self.flush(collection).await?;
    Ok(updated)
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    self.ensure_loaded(collection).await?;
    let mut w = self.cache.write().await;
    let records = match w.get_mut(collection) {
      Some(r) => r,
      None => return Ok(false),
    };

    let before = records.len();
    records.retain(|r| r.get("id").and_then(|v| v.as_str()) != Some(id));
    let removed = records.len() < before;
    drop(w);

    if removed {
      self.flush(collection).await?;
    }
    Ok(removed)
  }

  async fn count(&self, collection: &str, filter: Option<&crate::query::Filter>) -> OrmResult<u64> {
    self.ensure_loaded(collection).await?;
    let r = self.cache.read().await;
    let count = r
      .get(collection)
      .map(|recs| {
        recs
          .iter()
          .filter(|d| filter.map_or(true, |f| f.matches(d)))
          .count()
      })
      .unwrap_or(0);
    Ok(count as u64)
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<crate::query::Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    self.ensure_loaded(collection).await?;
    let mut w = self.cache.write().await;
    let records = w
      .get_mut(collection)
      .ok_or_else(|| OrmError::NotFound(format!("collection={}", collection)))?;

    let mut count = 0;
    for record in records.iter_mut() {
      if filter.as_ref().map_or(true, |f| f.matches(record)) {
        if let (Value::Object(base), Value::Object(patch)) = (record, &updates) {
          for (k, v) in patch {
            base.insert(k.clone(), v.clone());
          }
        }
        count += 1;
      }
    }
    drop(w);

    if count > 0 {
      self.flush(collection).await?;
    }
    Ok(count)
  }

  async fn delete_many(
    &self,
    collection: &str,
    filter: Option<crate::query::Filter>,
  ) -> OrmResult<usize> {
    self.ensure_loaded(collection).await?;
    let mut w = self.cache.write().await;
    let records = match w.get_mut(collection) {
      Some(r) => r,
      None => return Ok(0),
    };

    let before = records.len();
    records.retain(|r| filter.as_ref().map_or(false, |f| !f.matches(r)));
    let deleted = before - records.len();
    drop(w);

    if deleted > 0 {
      self.flush(collection).await?;
    }
    Ok(deleted)
  }

  async fn create_index(
    &self,
    _collection: &str,
    _index: &crate::nosql_index::NosqlIndex,
  ) -> OrmResult<()> {
    log::warn!("Indexes are not supported by the JSON provider");
    Ok(())
  }

  async fn drop_index(&self, _collection: &str, _index_name: &str) -> OrmResult<()> {
    log::warn!("Indexes are not supported by the JSON provider");
    Ok(())
  }

  async fn list_indexes(
    &self,
    _collection: &str,
  ) -> OrmResult<Vec<crate::nosql_index::NosqlIndexInfo>> {
    Ok(vec![])
  }
}

fn compare_values(a: Option<&Value>, b: Option<&Value>) -> std::cmp::Ordering {
  use std::cmp::Ordering;
  match (a, b) {
    (Some(Value::Number(n1)), Some(Value::Number(n2))) => n1
      .as_f64()
      .unwrap_or(0.0)
      .partial_cmp(&n2.as_f64().unwrap_or(0.0))
      .unwrap_or(Ordering::Equal),
    (Some(Value::String(s1)), Some(Value::String(s2))) => s1.cmp(s2),
    (Some(_), None) => Ordering::Greater,
    (None, Some(_)) => Ordering::Less,
    _ => Ordering::Equal,
  }
}

#[cfg(feature = "mongo")]
pub struct MongoPool {
  client: mongodb::Client,
  pool: Arc<PoolInner>,
}

#[cfg(feature = "mongo")]
impl MongoPool {
  pub async fn with_config(
    uri: impl AsRef<str>,
    db_name: impl AsRef<str>,
    config: PoolConfig,
  ) -> OrmResult<Self> {
    use mongodb::options::ClientOptions;
    let options = ClientOptions::parse(uri.as_ref())
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    let client =
      mongodb::Client::with_options(options).map_err(|e| OrmError::Connection(e.to_string()))?;
    Ok(Self {
      client,
      pool: Arc::new(PoolInner::new(config.max_size)),
    })
  }

  pub async fn acquire(&self, wait_for_available: bool) -> OrmResult<PooledMongo> {
    self.pool.acquire(wait_for_available).await?;
    Ok(PooledMongo {
      client: self.client.clone(),
      pool: Some(self.pool.clone()),
    })
  }

  pub fn client(&self) -> &mongodb::Client {
    &self.client
  }
}

#[cfg(feature = "mongo")]
#[derive(Clone)]
pub struct PooledMongo {
  client: mongodb::Client,
  pool: Option<Arc<PoolInner>>,
}

#[cfg(feature = "mongo")]
impl Drop for PooledMongo {
  fn drop(&mut self) {
    if let Some(pool) = self.pool.take() {
      pool.release();
    }
  }
}
