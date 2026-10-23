//! SettingsService — typed key-value settings store.
//!
//! Backed by a `JsonProvider` (or any other provider that supports
//! `find_by_id` / `insert` for a single "settings" collection).

use crate::provider::Storage;
use crate::providers::json::JsonProvider;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Errors for settings operations.
#[derive(Debug, thiserror::Error, Clone)]
pub enum SettingsError {
  #[error("provider error: {0}")]
  Provider(String),
  #[error("serialization error: {0}")]
  Serialization(String),
  #[error("not found: {0}")]
  NotFound(String),
}

/// SettingsService trait.
#[async_trait::async_trait]
pub trait SettingsService: Send + Sync {
  /// Get a setting by key. Returns None if not set.
  async fn get(&self, key: &str) -> Result<Option<Value>, SettingsError>;

  /// Set a setting (creates or updates).
  async fn set(&self, key: &str, value: Value) -> Result<(), SettingsError>;

  /// Delete a setting.
  async fn delete(&self, key: &str) -> Result<(), SettingsError>;

  /// List all keys.
  async fn keys(&self) -> Result<Vec<String>, SettingsError>;
}

/// JsonSettingsStore — settings backed by a `JsonProvider` in a
/// single "settings" collection.
pub struct JsonSettingsStore {
  provider: Arc<JsonProvider>,
  collection: String,
  cache: Arc<Mutex<HashMap<String, Value>>>,
}

impl JsonSettingsStore {
  /// Create a new JsonSettingsStore using the given provider.
  pub fn new(provider: Arc<JsonProvider>) -> Self {
    Self {
      provider,
      collection: "settings".to_string(),
      cache: Arc::new(Mutex::new(HashMap::new())),
    }
  }
}

#[async_trait::async_trait]
impl SettingsService for JsonSettingsStore {
  async fn get(&self, key: &str) -> Result<Option<Value>, SettingsError> {
    if let Some(v) = self.cache.lock().ok().and_then(|g| g.get(key).cloned()) {
      return Ok(Some(v));
    }
    match self
      .provider
      .find_by_id(&self.collection, key)
      .await
      .map_err(|e| SettingsError::Provider(e.to_string()))?
    {
      Some(v) => {
        if let Some(inner) = v.get("value") {
          if let Ok(mut g) = self.cache.lock() {
            g.insert(key.to_string(), inner.clone());
          }
          Ok(Some(inner.clone()))
        } else {
          Ok(None)
        }
      }
      None => Ok(None),
    }
  }

  async fn set(&self, key: &str, value: Value) -> Result<(), SettingsError> {
    if let Ok(mut g) = self.cache.lock() {
      g.insert(key.to_string(), value.clone());
    }
    let doc = serde_json::json!({ "value": value });
    self
      .provider
      .insert(&self.collection, doc)
      .await
      .map_err(|e| SettingsError::Provider(e.to_string()))?;
    Ok(())
  }

  async fn delete(&self, key: &str) -> Result<(), SettingsError> {
    if let Ok(mut g) = self.cache.lock() {
      g.remove(key);
    }
    self
      .provider
      .delete(&self.collection, key)
      .await
      .map_err(|e| SettingsError::Provider(e.to_string()))?;
    Ok(())
  }

  async fn keys(&self) -> Result<Vec<String>, SettingsError> {
    Ok(
      self
        .cache
        .lock()
        .ok()
        .map(|g| g.keys().cloned().collect())
        .unwrap_or_default(),
    )
  }
}
