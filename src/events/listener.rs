use crate::error::OrmResult;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::sync::{Arc, Weak};
use uuid::Uuid;

#[async_trait::async_trait]
pub trait EntityEventListener<E>: Send + Sync {
  async fn before_insert(&self, entity: &E) -> OrmResult<()>;
  async fn after_insert(&self, entity: &E) -> OrmResult<()>;
  async fn before_update(&self, entity: &E) -> OrmResult<()>;
  async fn after_update(&self, entity: &E) -> OrmResult<()>;
  async fn before_delete(&self, entity: &E) -> OrmResult<()>;
  async fn after_delete(&self, entity: &E) -> OrmResult<()>;
}

struct ListenerEntry {
  id: String,
  listener: Weak<dyn EntityEventListener<Value>>,
  created_at: DateTime<Utc>,
  ttl_seconds: Option<u64>,
}

impl ListenerEntry {
  fn is_expired(&self) -> bool {
    if let Some(ttl) = self.ttl_seconds {
      let age = Utc::now() - self.created_at;
      age.num_seconds() as u64 >= ttl
    } else {
      false
    }
  }
}

pub struct EntityEvents {
  listeners: Vec<ListenerEntry>,
  default_ttl_seconds: Option<u64>,
}

impl Default for EntityEvents {
  fn default() -> Self {
    Self::new()
  }
}

impl EntityEvents {
  pub fn new() -> Self {
    Self {
      listeners: Vec::new(),
      default_ttl_seconds: Some(3600),
    }
  }

  pub fn with_default_ttl(mut self, ttl_seconds: u64) -> Self {
    self.default_ttl_seconds = Some(ttl_seconds);
    self
  }

  fn cleanup_expired(&mut self) {
    self.listeners.retain(|entry| {
      if entry.is_expired() {
        false
      } else {
        entry.listener.upgrade().is_some()
      }
    });
  }

  pub fn add_listener<L: EntityEventListener<Value> + 'static>(
    &mut self,
    listener: L,
    ttl_seconds: Option<u64>,
  ) -> String {
    self.cleanup_expired();
    let id = Uuid::new_v4().to_string();
    let listener: Arc<dyn EntityEventListener<Value>> = Arc::new(listener);
    self.listeners.push(ListenerEntry {
      id: id.clone(),
      listener: Arc::downgrade(&listener),
      created_at: Utc::now(),
      ttl_seconds: ttl_seconds.or(self.default_ttl_seconds),
    });
    id
  }

  pub fn remove_listener(&mut self, id: &str) -> bool {
    let pos = self.listeners.iter().position(|e| e.id == id);
    if let Some(pos) = pos {
      self.listeners.remove(pos);
      true
    } else {
      false
    }
  }

  pub fn clear_listeners(&mut self) {
    self.listeners.clear();
  }

  pub fn listener_count(&self) -> usize {
    self.listeners.len()
  }

  pub async fn dispatch_insert(&self, entity: &Value) -> OrmResult<()> {
    for entry in &self.listeners {
      if let Some(listener) = entry.listener.upgrade() {
        listener.after_insert(entity).await?;
      }
    }
    Ok(())
  }

  pub async fn dispatch_update(&self, before: &Value, after: &Value) -> OrmResult<()> {
    for entry in &self.listeners {
      if let Some(listener) = entry.listener.upgrade() {
        listener.before_update(before).await?;
        listener.after_update(after).await?;
      }
    }
    Ok(())
  }

  pub async fn dispatch_delete(&self, entity: &Value) -> OrmResult<()> {
    for entry in &self.listeners {
      if let Some(listener) = entry.listener.upgrade() {
        listener.before_delete(entity).await?;
        listener.after_delete(entity).await?;
      }
    }
    Ok(())
  }
}
