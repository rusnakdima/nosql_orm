use crate::error::{OrmError, OrmResult};
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

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
  access_order: u64,
}

impl Debug for CachedEntry {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CachedEntry")
      .field("cached_at", &self.cached_at)
      .field("expires_at", &self.expires_at)
      .field("access_order", &self.access_order)
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

#[derive(Debug)]
struct CacheState {
  entries: std::collections::HashMap<String, CachedEntry>,
  access_order: BTreeMap<u64, String>,
  stats: CacheStats,
  next_access_order: u64,
}

impl Default for CacheState {
  fn default() -> Self {
    Self {
      entries: std::collections::HashMap::new(),
      access_order: BTreeMap::new(),
      stats: CacheStats::default(),
      next_access_order: 0,
    }
  }
}

#[derive(Debug, Clone)]
pub struct QueryCache {
  config: CacheConfig,
  state: Arc<RwLock<CacheState>>,
}

impl QueryCache {
  pub fn new(config: CacheConfig) -> Self {
    Self {
      config,
      state: Arc::new(RwLock::new(CacheState::default())),
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
    let mut state = self.state.write().await;

    let (entry_data, old_order) = {
      let entry = match state.entries.get_mut(key) {
        Some(e) => e,
        None => {
          state.stats.misses += 1;
          return Ok(None);
        }
      };

      let should_remove = if let Some(expires) = entry.expires_at {
        Utc::now() > expires
      } else {
        false
      };

      if should_remove {
        state.entries.remove(key);
        state.access_order.retain(|_, k| k != key);
        state.stats.misses += 1;
        return Ok(None);
      }

      (entry.data.clone(), entry.access_order)
    };

    let new_order = state.next_access_order;
    state.next_access_order += 1;
    state.access_order.remove(&old_order);
    state.access_order.insert(new_order, key.to_string());
    state.stats.hits += 1;

    let result = serde_json::from_value(entry_data)?;
    Ok(Some(result))
  }

  pub async fn set<T: Serialize>(&self, key: String, data: &T) -> OrmResult<()> {
    let mut state = self.state.write().await;

    if state.entries.len() >= self.config.max_size && !state.entries.contains_key(&key) {
      if let Some((oldest_order, oldest_key)) = state.access_order.pop_first() {
        state.entries.remove(&oldest_key);
        state.stats.evictions += 1;
      }
    }

    let value = serde_json::to_value(data).map_err(OrmError::Serialization)?;
    let now = Utc::now();
    let expires_at = self
      .config
      .ttl_secs
      .map(|secs| now + chrono::Duration::seconds(secs as i64));

    let new_order = state.next_access_order;
    state.next_access_order += 1;

    let old_order = state.entries.get(&key).map(|e| e.access_order);

    if let Some(old_entry) = state.entries.get_mut(&key) {
      old_entry.data = value;
      old_entry.cached_at = now;
      old_entry.expires_at = expires_at;
      old_entry.access_order = new_order;
    } else {
      state.entries.insert(
        key.clone(),
        CachedEntry {
          data: value,
          cached_at: now,
          expires_at,
          access_order: new_order,
        },
      );
    }

    if let Some(old) = old_order {
      state.access_order.remove(&old);
    }
    state.access_order.insert(new_order, key);

    Ok(())
  }

  pub async fn invalidate_collection(&self, collection: &str) -> OrmResult<()> {
    let prefix = format!("{}|{}|", self.config.key_prefix, collection);
    let mut state = self.state.write().await;

    let keys_to_remove: Vec<String> = state
      .entries
      .keys()
      .filter(|k| k.starts_with(&prefix))
      .cloned()
      .collect();

    for key in keys_to_remove {
      state.entries.remove(&key);
      state.access_order.retain(|_, k| k != &key);
    }

    Ok(())
  }

  pub async fn invalidate(&self, key: &str) -> OrmResult<()> {
    let mut state = self.state.write().await;
    state.entries.remove(key);
    state.access_order.retain(|_, k| k != key);
    Ok(())
  }

  pub async fn clear(&self) -> OrmResult<()> {
    let mut state = self.state.write().await;
    state.stats.evictions += state.entries.len() as u64;
    state.entries.clear();
    state.access_order.clear();
    state.stats.entries = 0;
    Ok(())
  }

  pub async fn stats(&self) -> CacheStats {
    let state = self.state.read().await;
    CacheStats {
      entries: state.entries.len(),
      hits: state.stats.hits,
      misses: state.stats.misses,
      evictions: state.stats.evictions,
    }
  }
}
