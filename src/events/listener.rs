use crate::error::OrmResult;
use serde_json::Value;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait EntityEventListener<E>: Send + Sync {
  async fn before_insert(&self, entity: &E) -> OrmResult<()>;
  async fn after_insert(&self, entity: &E) -> OrmResult<()>;
  async fn before_update(&self, entity: &E) -> OrmResult<()>;
  async fn after_update(&self, entity: &E) -> OrmResult<()>;
  async fn before_delete(&self, entity: &E) -> OrmResult<()>;
  async fn after_delete(&self, entity: &E) -> OrmResult<()>;
}

pub struct EntityEvents {
  pub listeners: Vec<Arc<dyn EntityEventListener<Value>>>,
}

impl EntityEvents {
  pub fn new() -> Self {
    Self {
      listeners: Vec::new(),
    }
  }

  pub fn add_listener<L: EntityEventListener<Value> + 'static>(&mut self, listener: L) {
    self.listeners.push(Arc::new(listener));
  }

  pub async fn dispatch_insert(&self, entity: &Value) -> OrmResult<()> {
    for listener in &self.listeners {
      listener.after_insert(entity).await?;
    }
    Ok(())
  }

  pub async fn dispatch_update(&self, before: &Value, after: &Value) -> OrmResult<()> {
    for listener in &self.listeners {
      listener.before_update(before).await?;
      listener.after_update(after).await?;
    }
    Ok(())
  }

  pub async fn dispatch_delete(&self, entity: &Value) -> OrmResult<()> {
    for listener in &self.listeners {
      listener.before_delete(entity).await?;
      listener.after_delete(entity).await?;
    }
    Ok(())
  }
}
