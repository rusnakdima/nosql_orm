use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub trait Timestamps {
  fn created_at(&self) -> Option<DateTime<Utc>>;
  fn updated_at(&self) -> Option<DateTime<Utc>>;
  fn set_created_at(&mut self, ts: DateTime<Utc>);
  fn set_updated_at(&mut self, ts: DateTime<Utc>);
  fn apply_timestamps_for_insert(&mut self);
  fn apply_timestamps_for_update(&mut self);
}

impl Timestamps for Value {
  fn created_at(&self) -> Option<DateTime<Utc>> {
    self
      .get("created_at")
      .and_then(|v| v.as_str())
      .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
      .map(|dt| dt.with_timezone(&Utc))
  }

  fn updated_at(&self) -> Option<DateTime<Utc>> {
    self
      .get("updated_at")
      .and_then(|v| v.as_str())
      .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
      .map(|dt| dt.with_timezone(&Utc))
  }

  fn set_created_at(&mut self, ts: DateTime<Utc>) {
    if let Some(obj) = self.as_object_mut() {
      obj.insert(
        "created_at".to_string(),
        serde_json::Value::String(ts.to_rfc3339()),
      );
    }
  }

  fn set_updated_at(&mut self, ts: DateTime<Utc>) {
    if let Some(obj) = self.as_object_mut() {
      obj.insert(
        "updated_at".to_string(),
        serde_json::Value::String(ts.to_rfc3339()),
      );
    }
  }

  fn apply_timestamps_for_insert(&mut self) {
    let now = Utc::now();
    if let Some(obj) = self.as_object_mut() {
      if !obj.contains_key("created_at") {
        obj.insert(
          "created_at".to_string(),
          serde_json::Value::String(now.to_rfc3339()),
        );
      }
      if !obj.contains_key("updated_at") {
        obj.insert(
          "updated_at".to_string(),
          serde_json::Value::String(now.to_rfc3339()),
        );
      }
    }
  }

  fn apply_timestamps_for_update(&mut self) {
    let now = Utc::now();
    if let Some(obj) = self.as_object_mut() {
      obj.insert(
        "updated_at".to_string(),
        serde_json::Value::String(now.to_rfc3339()),
      );
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampFields {
  pub created_at: Option<DateTime<Utc>>,
  pub updated_at: Option<DateTime<Utc>>,
}

impl TimestampFields {
  pub fn now() -> Self {
    let now = Utc::now();
    Self {
      created_at: Some(now),
      updated_at: Some(now),
    }
  }

  pub fn touch(&mut self) {
    self.updated_at = Some(Utc::now());
  }

  pub fn to_value(&self) -> Value {
    serde_json::json!({
      "created_at": self.created_at,
      "updated_at": self.updated_at,
    })
  }
}

pub fn apply_timestamps(doc: &mut Value, is_insert: bool) {
  let now = Utc::now().to_rfc3339();
  if is_insert {
    if let Some(obj) = doc.as_object_mut() {
      if !obj.contains_key("created_at") {
        obj.insert("created_at".to_string(), Value::String(now.clone()));
      }
      if !obj.contains_key("updated_at") {
        obj.insert("updated_at".to_string(), Value::String(now));
      }
    }
  } else if let Some(obj) = doc.as_object_mut() {
    obj.insert("updated_at".to_string(), Value::String(now));
  }
}

pub fn timestamp_now_rfc3339() -> String {
  Utc::now().to_rfc3339()
}
