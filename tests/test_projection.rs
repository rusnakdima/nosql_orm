use nosql_orm::query::Projection;
use serde_json::json;

#[test]
fn test_projection_select_single_field() {
  let proj = Projection::select(&["name"]);
  let doc = json!({"id": "1", "name": "Alice", "email": "alice@example.com"});
  let result = proj.apply(&doc);

  let obj = result.as_object().unwrap();
  assert!(obj.contains_key("name"));
  assert!(!obj.contains_key("id"));
  assert!(!obj.contains_key("email"));
}

#[test]
fn test_projection_select_multiple_fields() {
  let proj = Projection::select(&["id", "name", "email"]);
  let doc = json!({"id": "1", "name": "Alice", "email": "alice@example.com", "age": 30});
  let result = proj.apply(&doc);

  let obj = result.as_object().unwrap();
  assert_eq!(obj.len(), 3);
  assert!(obj.contains_key("id"));
  assert!(obj.contains_key("name"));
  assert!(obj.contains_key("email"));
  assert!(!obj.contains_key("age"));
}

#[test]
fn test_projection_exclude_single_field() {
  let proj = Projection::exclude(&["password"]);
  let doc = json!({"id": "1", "name": "Alice", "password": "secret123"});
  let result = proj.apply(&doc);

  let obj = result.as_object().unwrap();
  assert_eq!(obj.len(), 2);
  assert!(obj.contains_key("id"));
  assert!(obj.contains_key("name"));
  assert!(!obj.contains_key("password"));
}

#[test]
fn test_projection_exclude_multiple_fields() {
  let proj = Projection::exclude(&["password", "token", "ssn"]);
  let doc = json!({
      "id": "1",
      "name": "Alice",
      "password": "secret",
      "token": "abc",
      "ssn": "123-45-6789"
  });
  let result = proj.apply(&doc);

  let obj = result.as_object().unwrap();
  assert_eq!(obj.len(), 2);
  assert!(obj.contains_key("id"));
  assert!(obj.contains_key("name"));
}

#[test]
fn test_projection_empty_select() {
  let proj = Projection::new();
  let doc = json!({"id": "1", "name": "Alice"});
  let result = proj.apply(&doc);

  assert_eq!(result, doc);
}

#[test]
fn test_projection_is_empty() {
  let empty = Projection::new();
  assert!(empty.is_empty());

  let select = Projection::select(&["id"]);
  assert!(!select.is_empty());

  let exclude = Projection::exclude(&["password"]);
  assert!(!exclude.is_empty());
}

#[test]
fn test_projection_apply_to_non_object() {
  let proj = Projection::select(&["name"]);
  let doc = json!("string value");
  let result = proj.apply(&doc);

  assert_eq!(result, doc);
}

#[test]
fn test_projection_select_nonexistent_field() {
  let proj = Projection::select(&["name", "nonexistent"]);
  let doc = json!({"id": "1", "name": "Alice"});
  let result = proj.apply(&doc);

  let obj = result.as_object().unwrap();
  assert!(obj.contains_key("name"));
  assert!(!obj.contains_key("nonexistent"));
  assert!(!obj.contains_key("id")); // only selected fields are returned
}

#[test]
fn test_projection_exclude_nonexistent_field() {
  let proj = Projection::exclude(&["nonexistent"]);
  let doc = json!({"id": "1", "name": "Alice"});
  let result = proj.apply(&doc);

  let obj = result.as_object().unwrap();
  assert_eq!(obj.len(), 2);
}

#[test]
fn test_projection_preserves_original() {
  let proj = Projection::select(&["name"]);
  let doc = json!({"id": "1", "name": "Alice", "email": "alice@example.com"});
  let _result = proj.apply(&doc);

  let obj = doc.as_object().unwrap();
  assert!(obj.contains_key("id"));
  assert!(obj.contains_key("name"));
  assert!(obj.contains_key("email"));
}
