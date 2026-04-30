use crate::error::{OrmError, OrmResult};
use serde_json::Value;
use uuid::Uuid;

pub trait DocumentExt {
  fn inject_collection(self, collection: &str) -> Self;
  fn get_collection(&self) -> Option<&str>;
}

impl DocumentExt for Value {
  fn inject_collection(mut self, collection: &str) -> Self {
    if let Some(obj) = self.as_object_mut() {
      obj.insert(
        "_collection".to_string(),
        Value::String(collection.to_string()),
      );
    }
    self
  }

  fn get_collection(&self) -> Option<&str> {
    self.get("_collection").and_then(|v| v.as_str())
  }
}

pub fn get_document_id(doc: &Value) -> Option<&str> {
  doc.get("id").and_then(|v| v.as_str())
}

pub fn get_document_id_string(doc: &Value) -> OrmResult<String> {
  get_document_id(doc)
    .map(String::from)
    .ok_or_else(|| OrmError::Validation("Missing or invalid 'id' field".to_string()))
}

/// Generate a new random UUIDv4 string.
pub fn generate_id() -> String {
  Uuid::new_v4().to_string()
}

/// Generate a short 8-character id suitable for display.
pub fn short_id() -> String {
  Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
}

/// Compare two optional JSON values for ordering.
pub fn compare_values(a: Option<&Value>, b: Option<&Value>) -> std::cmp::Ordering {
  use std::cmp::Ordering;
  match (a, b) {
    (Some(Value::Number(n1)), Some(Value::Number(n2))) => n1
      .as_f64()
      .unwrap_or(0.0)
      .partial_cmp(&n2.as_f64().unwrap_or(0.0))
      .unwrap_or(Ordering::Equal),
    (Some(Value::String(s1)), Some(Value::String(s2))) => s1.cmp(s2),
    (Some(_), None) => Ordering::Greater,
    (None, Some(_)) => Ordering::Less,
    _ => Ordering::Equal,
  }
}
