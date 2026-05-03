use crate::error::{map_err_connection, OrmError, OrmResult};
use crate::provider::{DatabaseProvider, IndexInfo};
use crate::providers::json::JsonProvider;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

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
  _permit: Option<OwnedSemaphorePermit>,
}

impl<T> Pooled<T> {
  pub fn new(inner: T) -> Self {
    Self {
      inner,
      pool: None,
      _permit: None,
    }
  }

  pub fn from_pool(inner: T, pool: Arc<PoolInner>, permit: OwnedSemaphorePermit) -> Self {
    Self {
      inner,
      pool: Some(pool),
      _permit: Some(permit),
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
      drop(self._permit.take());
      pool.release();
    }
  }
}

pub struct PoolInner {
  semaphore: Arc<Semaphore>,
  available: std::sync::atomic::AtomicUsize,
  #[allow(dead_code)]
  total: std::sync::atomic::AtomicUsize,
}

impl PoolInner {
  fn new(max_size: usize) -> Self {
    Self {
      semaphore: Arc::new(Semaphore::new(max_size)),
      available: std::sync::atomic::AtomicUsize::new(max_size),
      total: std::sync::atomic::AtomicUsize::new(max_size),
    }
  }

  async fn acquire(&self, wait_for_available: bool) -> OrmResult<OwnedSemaphorePermit> {
    let semaphore = self.semaphore.clone();
    let permit = if wait_for_available {
      semaphore
        .acquire_owned()
        .await
        .map_err(|_| OrmError::Connection("Pool acquire failed".to_string()))?
    } else {
      semaphore
        .try_acquire_owned()
        .map_err(|_| OrmError::Connection("No available connections in pool".to_string()))?
    };
    self
      .available
      .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    Ok(permit)
  }

  fn release(&self) {
    self
      .available
      .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    self.semaphore.add_permits(1);
  }
}

pub struct Pool<P: DatabaseProvider> {
  provider: Arc<P>,
  inner: Arc<PoolInner>,
}

impl<P: DatabaseProvider> Pool<P> {
  pub fn with_config(provider: P, config: PoolConfig) -> Self {
    Self {
      provider: Arc::new(provider),
      inner: Arc::new(PoolInner::new(config.max_size)),
    }
  }

  pub async fn acquire(&self, wait_for_available: bool) -> OrmResult<Pooled<P>> {
    let permit = self.inner.acquire(wait_for_available).await?;
    Ok(Pooled::from_pool(
      (*self.provider).clone(),
      self.inner.clone(),
      permit,
    ))
  }
}

#[derive(Clone)]
pub struct JsonPool {
  provider: Arc<JsonProvider>,
  pool: Arc<PoolInner>,
}

impl JsonPool {
  pub async fn with_config(base_dir: std::path::PathBuf, config: PoolConfig) -> OrmResult<Self> {
    let provider = JsonProvider::new(base_dir).await?;
    Ok(Self {
      provider: Arc::new(provider),
      pool: Arc::new(PoolInner::new(config.max_size)),
    })
  }

  pub async fn acquire(&self, wait_for_available: bool) -> OrmResult<PooledJson> {
    let _ = self.pool.acquire(wait_for_available).await?;
    Ok(PooledJson {
      provider: self.provider.clone(),
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
  provider: Arc<JsonProvider>,
  pool: Option<Arc<PoolInner>>,
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
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    self.provider.insert(collection, doc).await
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    self.provider.find_by_id(collection, id).await
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
      .provider
      .find_many(collection, filter, skip, limit, sort_by, sort_asc)
      .await
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    self.provider.update(collection, id, doc).await
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    self.provider.patch(collection, id, patch).await
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    self.provider.delete(collection, id).await
  }

  async fn count(&self, collection: &str, filter: Option<&crate::query::Filter>) -> OrmResult<u64> {
    self.provider.count(collection, filter).await
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<crate::query::Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    self.provider.update_many(collection, filter, updates).await
  }

  async fn delete_many(
    &self,
    collection: &str,
    filter: Option<crate::query::Filter>,
  ) -> OrmResult<usize> {
    self.provider.delete_many(collection, filter).await
  }

  async fn create_index(
    &self,
    collection: &str,
    index: &crate::nosql_index::NosqlIndex,
  ) -> OrmResult<()> {
    self.provider.create_index(collection, index).await
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    self.provider.drop_index(collection, index_name).await
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    self.provider.list_indexes(collection).await
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
    _db_name: impl AsRef<str>,
    config: PoolConfig,
  ) -> OrmResult<Self> {
    let options = map_err_connection(mongodb::options::ClientOptions::parse(uri.as_ref()).await)?;
    let client = map_err_connection(mongodb::Client::with_options(options))?;
    Ok(Self {
      client,
      pool: Arc::new(PoolInner::new(config.max_size)),
    })
  }

  pub async fn acquire(&self, wait_for_available: bool) -> OrmResult<PooledMongo> {
    let _permit = self.pool.acquire(wait_for_available).await?;
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
  #[allow(dead_code)]
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
