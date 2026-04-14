use serde_json::Value;

#[derive(Debug, Clone)]
pub enum EventType {
  BeforeInsert,
  AfterInsert,
  BeforeUpdate,
  AfterUpdate,
  BeforeDelete,
  AfterDelete,
  BeforeQuery,
  AfterQuery,
}

pub struct Event {
  pub event_type: EventType,
  pub entity_type: String,
  pub data: Value,
  pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Event {
  pub fn new(event_type: EventType, entity_type: &str, data: Value) -> Self {
    Self {
      event_type,
      entity_type: entity_type.to_string(),
      data,
      timestamp: chrono::Utc::now(),
    }
  }
}

pub struct InsertEvent {
  pub entity: Value,
  pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct UpdateEvent {
  pub before: Value,
  pub after: Value,
  pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct DeleteEvent {
  pub entity: Value,
  pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct QueryEvent {
  pub query: Value,
  pub result_count: usize,
  pub timestamp: chrono::DateTime<chrono::Utc>,
}
