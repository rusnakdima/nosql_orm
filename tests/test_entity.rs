use nosql_orm::entity::extract_id;
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SimpleEntity {
  id: Option<String>,
  name: String,
}

impl SimpleEntity {
  fn new(name: &str) -> Self {
    Self {
      id: None,
      name: name.to_string(),
    }
  }
}

impl Entity for SimpleEntity {
  fn meta() -> EntityMeta {
    EntityMeta::new("simple_entities")
  }
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }
  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }
}

#[test]
fn test_entity_meta_new() {
  let meta = EntityMeta::new("users");
  assert_eq!(meta.table_name, "users");
  assert_eq!(meta.id_field, "id");
  assert!(meta.sql_columns.is_none());
}

#[test]
fn test_entity_meta_with_id_field() {
  let meta = EntityMeta::new("users").with_id_field("user_id");
  assert_eq!(meta.id_field, "user_id");
}

#[test]
fn test_entity_meta_with_sql_columns() {
  use nosql_orm::sql::{SqlColumnDef, SqlColumnType};
  let columns = vec![
    SqlColumnDef::new("id", SqlColumnType::Integer).primary_key(),
    SqlColumnDef::new("name", SqlColumnType::Text),
  ];
  let meta = EntityMeta::new("users").with_sql_columns(columns);
  assert!(meta.sql_columns.is_some());
  assert_eq!(meta.sql_columns.as_ref().unwrap().len(), 2);
}

#[test]
fn test_entity_meta_sql_table_name() {
  let meta = EntityMeta::new("users");
  assert_eq!(meta.sql_table_name(), "users");
}

#[test]
fn test_entity_get_id() {
  let mut entity = SimpleEntity::new("Alice");
  assert!(entity.get_id().is_none());

  entity.set_id("123".to_string());
  assert_eq!(entity.get_id(), Some("123".to_string()));
}

#[test]
fn test_entity_table_name() {
  assert_eq!(SimpleEntity::table_name(), "simple_entities");
}

#[test]
fn test_entity_is_soft_deletable_default() {
  assert!(!SimpleEntity::is_soft_deletable());
}

#[test]
fn test_entity_indexes_default() {
  let indexes = SimpleEntity::indexes();
  assert!(indexes.is_empty());
}

#[test]
fn test_entity_sql_columns_default() {
  let columns = SimpleEntity::sql_columns();
  assert!(columns.is_empty());
}

#[test]
fn test_entity_to_value() {
  let mut entity = SimpleEntity::new("Alice");
  entity.set_id("123".to_string());
  let value = entity.to_value().unwrap();

  assert_eq!(value["id"], "123");
  assert_eq!(value["name"], "Alice");
}

#[test]
fn test_entity_from_value() {
  let value = json!({"id": "123", "name": "Alice"});
  let entity = SimpleEntity::from_value(value).unwrap();

  assert_eq!(entity.get_id(), Some("123".to_string()));
  assert_eq!(entity.name, "Alice");
}

#[test]
fn test_extract_id() {
  let doc = json!({"id": "abc123", "name": "Alice"});
  assert_eq!(extract_id(&doc, "id"), Some("abc123".to_string()));

  let doc_no_id = json!({"name": "Alice"});
  assert_eq!(extract_id(&doc_no_id, "id"), None);

  let doc_null = json!({"id": null, "name": "Alice"});
  assert_eq!(extract_id(&doc_null, "id"), None);
}

#[test]
fn test_entity_roundtrip() {
  let mut entity = SimpleEntity::new("Bob");
  entity.set_id("456".to_string());

  let value = entity.to_value().unwrap();
  let restored = SimpleEntity::from_value(value).unwrap();

  assert_eq!(restored.get_id(), entity.get_id());
  assert_eq!(restored.name, entity.name);
}
