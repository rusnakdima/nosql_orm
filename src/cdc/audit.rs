use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
  Create,
  Read,
  Update,
  Delete,
  Login,
  Logout,
  Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
  pub id: String,
  pub action: AuditAction,
  pub entity_type: Option<String>,
  pub entity_id: Option<String>,
  pub user_id: Option<String>,
  pub session_id: Option<String>,
  pub ip_address: Option<String>,
  pub user_agent: Option<String>,
  pub request_id: Option<String>,
  pub changes: Option<serde_json::Value>,
  pub metadata: Option<serde_json::Value>,
  pub timestamp: DateTime<Utc>,
}

impl AuditLog {
  pub fn new(action: AuditAction) -> Self {
    Self {
      id: uuid::Uuid::new_v4().to_string(),
      action,
      entity_type: None,
      entity_id: None,
      user_id: None,
      session_id: None,
      ip_address: None,
      user_agent: None,
      request_id: None,
      changes: None,
      metadata: None,
      timestamp: Utc::now(),
    }
  }

  pub fn entity(mut self, entity_type: &str, entity_id: &str) -> Self {
    self.entity_type = Some(entity_type.to_string());
    self.entity_id = Some(entity_id.to_string());
    self
  }

  pub fn user(mut self, user_id: &str) -> Self {
    self.user_id = Some(user_id.to_string());
    self
  }

  pub fn changes(mut self, changes: serde_json::Value) -> Self {
    self.changes = Some(changes);
    self
  }
}
