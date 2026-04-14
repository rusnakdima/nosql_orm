use crate::error::OrmResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
  Insert,
  Update,
  Delete,
  SoftDelete,
  Restore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
  pub id: String,
  pub change_type: ChangeType,
  pub collection: String,
  pub entity_id: String,
  pub before: Option<serde_json::Value>,
  pub after: Option<serde_json::Value>,
  pub timestamp: DateTime<Utc>,
  pub user_id: Option<String>,
  pub trace_id: Option<String>,
}

impl Change {
  pub fn insert(collection: &str, entity_id: &str, data: serde_json::Value) -> Self {
    Self {
      id: uuid::Uuid::new_v4().to_string(),
      change_type: ChangeType::Insert,
      collection: collection.to_string(),
      entity_id: entity_id.to_string(),
      before: None,
      after: Some(data),
      timestamp: Utc::now(),
      user_id: None,
      trace_id: None,
    }
  }

  pub fn update(
    collection: &str,
    entity_id: &str,
    before: serde_json::Value,
    after: serde_json::Value,
  ) -> Self {
    Self {
      id: uuid::Uuid::new_v4().to_string(),
      change_type: ChangeType::Update,
      collection: collection.to_string(),
      entity_id: entity_id.to_string(),
      before: Some(before),
      after: Some(after),
      timestamp: Utc::now(),
      user_id: None,
      trace_id: None,
    }
  }

  pub fn delete(collection: &str, entity_id: &str, data: serde_json::Value) -> Self {
    Self {
      id: uuid::Uuid::new_v4().to_string(),
      change_type: ChangeType::Delete,
      collection: collection.to_string(),
      entity_id: entity_id.to_string(),
      before: Some(data),
      after: None,
      timestamp: Utc::now(),
      user_id: None,
      trace_id: None,
    }
  }
}

#[async_trait::async_trait]
pub trait ChangeCapture: Send + Sync {
  async fn capture(&self, change: Change) -> OrmResult<()>;
  async fn get_changes(
    &self,
    collection: &str,
    since: chrono::DateTime<Utc>,
  ) -> OrmResult<Vec<Change>>;
  async fn get_entity_history(&self, collection: &str, entity_id: &str) -> OrmResult<Vec<Change>>;
}
