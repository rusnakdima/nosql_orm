use serde_json::Value;
use uuid::Uuid;

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
