use crate::error::OrmResult;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
  pub max_size: usize,
  pub ttl_secs: Option<u64>,
  pub key_prefix: String,
}

impl Default for CacheConfig {
  fn default() -> Self {
    Self {
      max_size: 1000,
      ttl_secs: Some(300),
      key_prefix: "nosql_orm".to_string(),
    }
  }
}

/// A cached query result
#[derive(Debug, Clone)]
pub struct CachedResult<T: Clone> {
  pub data: T,
  pub cached_at: DateTime<Utc>,
  pub expires_at: Option<DateTime<Utc>>,
}

impl<T: Clone> CachedResult<T> {
  pub fn is_expired(&self) -> bool {
    if let Some(expires) = self.expires_at {
      return Utc::now() > expires;
    }
    false
  }
}

struct CachedEntry {
  data: serde_json::Value,
  cached_at: DateTime<Utc>,
  expires_at: Option<DateTime<Utc>>,
}

impl Debug for CachedEntry {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CachedEntry")
      .field("cached_at", &self.cached_at)
      .field("expires_at", &self.expires_at)
      .finish()
  }
}

#[derive(Debug, Clone, Default)]
pub struct CacheStats {
  pub entries: usize,
  pub hits: u64,
  pub misses: u64,
  pub evictions: u64,
}

#[derive(Debug, Clone)]
pub struct QueryCache {
  config: CacheConfig,
  entries: Arc<RwLock<HashMap<String, CachedEntry>>>,
  access_order: Arc<RwLock<Vec<String>>>,
  stats: Arc<RwLock<CacheStats>>,
}

impl QueryCache {
  pub fn new(config: CacheConfig) -> Self {
    Self {
      config,
      entries: Arc::new(RwLock::new(HashMap::new())),
      access_order: Arc::new(RwLock::new(Vec::new())),
      stats: Arc::new(RwLock::new(CacheStats::default())),
    }
  }

  pub fn cache_key(
    &self,
    collection: &str,
    filter_json: Option<&str>,
    skip: Option<u64>,
    limit: Option<u64>,
    order_by: Option<&str>,
  ) -> String {
    let mut parts = vec![self.config.key_prefix.clone(), collection.to_string()];
    if let Some(f) = filter_json {
      parts.push(f.to_string());
    }
    if let Some(s) = skip {
      parts.push(format!("s:{}", s));
    }
    if let Some(l) = limit {
      parts.push(format!("l:{}", l));
    }
    if let Some(o) = order_by {
      parts.push(format!("o:{}", o));
    }
    parts.join("|")
  }

  pub async fn get<T: DeserializeOwned>(&self, key: &str) -> OrmResult<Option<T>> {
    let mut entries = self.entries.write().await;
    let mut access_order = self.access_order.write().await;

    if let Some(entry) = entries.get(key) {
      if let Some(expires) = entry.expires_at {
        if Utc::now() > expires {
          entries.remove(key);
          access_order.retain(|k| k != key);
          drop(entries);
          drop(access_order);
          let mut stats = self.stats.write().await;
          stats.misses += 1;
          return Ok(None);
        }
      }

      if let Some(pos) = access_order.iter().position(|k| k == key) {
        access_order.remove(pos);
      }
      access_order.push(key.to_string());

      let mut stats = self.stats.write().await;
      stats.hits += 1;

      let result = serde_json::from_value(entry.data.clone())?;
      return Ok(Some(result));
    }

    drop(access_order);
    let mut stats = self.stats.write().await;
    stats.misses += 1;
    Ok(None)
  }

  pub async fn set<T: Serialize>(&self, key: String, data: &T) -> OrmResult<()> {
    let mut entries = self.entries.write().await;
    let mut access_order = self.access_order.write().await;

    if entries.len() >= self.config.max_size && !entries.contains_key(&key) {
      if let Some(oldest) = access_order.first().cloned() {
        entries.remove(&oldest);
        access_order.remove(0);
        let mut stats = self.stats.write().await;
        stats.evictions += 1;
      }
    }

    let value = serde_json::to_value(data)?;
    let now = Utc::now();
    let expires_at = self
      .config
      .ttl_secs
      .map(|secs| now + chrono::Duration::seconds(secs as i64));

    entries.insert(
      key.clone(),
      CachedEntry {
        data: value,
        cached_at: now,
        expires_at,
      },
    );

    if let Some(pos) = access_order.iter().position(|k| k == &key) {
      access_order.remove(pos);
    }
    access_order.push(key);

    Ok(())
  }

  pub async fn invalidate_collection(&self, collection: &str) -> OrmResult<()> {
    let prefix = format!("{}|{}|", self.config.key_prefix, collection);
    let mut entries = self.entries.write().await;
    let mut access_order = self.access_order.write().await;

    let keys_to_remove: Vec<String> = entries
      .keys()
      .filter(|k| k.starts_with(&prefix))
      .cloned()
      .collect();

    for key in keys_to_remove {
      entries.remove(&key);
      access_order.retain(|k| k != &key);
    }

    Ok(())
  }

  pub async fn invalidate(&self, key: &str) -> OrmResult<()> {
    let mut entries = self.entries.write().await;
    let mut access_order = self.access_order.write().await;
    entries.remove(key);
    access_order.retain(|k| k != key);
    Ok(())
  }

  pub async fn clear(&self) -> OrmResult<()> {
    let mut entries = self.entries.write().await;
    let mut access_order = self.access_order.write().await;
    let mut stats = self.stats.write().await;
    entries.clear();
    access_order.clear();
    stats.evictions += stats.entries as u64;
    stats.entries = 0;
    Ok(())
  }

  pub async fn stats(&self) -> CacheStats {
    let entries = self.entries.read().await;
    let stats = self.stats.read().await;
    CacheStats {
      entries: entries.len(),
      hits: stats.hits,
      misses: stats.misses,
      evictions: stats.evictions,
    }
  }
}
