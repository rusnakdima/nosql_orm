use nosql_orm::timestamps::{apply_timestamps, timestamp_now_rfc3339, TimestampFields};
use serde_json::json;

#[test]
fn test_apply_timestamps_on_insert() {
  let mut doc = json!({"name": "Alice", "email": "alice@example.com"});
  apply_timestamps(&mut doc, true);

  let obj = doc.as_object().unwrap();
  assert!(obj.contains_key("created_at"));
  assert!(obj.contains_key("updated_at"));
}

#[test]
fn test_apply_timestamps_on_update() {
  let mut doc = json!({"name": "Alice", "created_at": "2024-01-01T00:00:00Z"});
  let before = doc["updated_at"].clone();
  apply_timestamps(&mut doc, false);

  let obj = doc.as_object().unwrap();
  assert!(obj.contains_key("updated_at"));
  assert_ne!(before, doc["updated_at"]);
}

#[test]
fn test_apply_timestamps_does_not_overwrite_existing() {
  let mut doc = json!({
      "name": "Alice",
      "created_at": "2024-01-01T00:00:00Z",
      "updated_at": "2024-01-01T00:00:00Z"
  });
  apply_timestamps(&mut doc, true);

  assert_eq!(doc["created_at"], "2024-01-01T00:00:00Z");
  assert_eq!(doc["updated_at"], "2024-01-01T00:00:00Z");
}

#[test]
fn test_apply_timestamps_update_overwrites() {
  let mut doc = json!({"name": "Alice"});
  apply_timestamps(&mut doc, true);

  let first_updated = doc["updated_at"].clone();
  apply_timestamps(&mut doc, false);
  assert_ne!(first_updated, doc["updated_at"]);
}

#[test]
fn test_timestamp_now_rfc3339_format() {
  let ts = timestamp_now_rfc3339();
  assert!(ts.contains("T"));
}

#[test]
fn test_timestamp_fields_now() {
  let tf = TimestampFields::now();
  assert!(tf.created_at.is_some());
  assert!(tf.updated_at.is_some());
}

#[test]
fn test_timestamp_fields_touch() {
  let mut tf = TimestampFields::now();
  let original_updated = tf.updated_at;
  std::thread::sleep(std::time::Duration::from_millis(10));
  tf.touch();
  assert!(tf.updated_at > original_updated);
}

#[test]
fn test_timestamp_fields_to_value() {
  let tf = TimestampFields::now();
  let value = tf.to_value();
  assert!(value.is_object());
  let obj = value.as_object().unwrap();
  assert!(obj.contains_key("created_at"));
  assert!(obj.contains_key("updated_at"));
}
