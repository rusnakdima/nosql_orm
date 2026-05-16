use std::collections::HashSet;

#[derive(Clone, Copy)]
pub enum CascadeAction {
  SoftDelete,
  Restore,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CascadeEntityRef {
  pub id: String,
  pub collection: String,
}

impl CascadeEntityRef {
  pub fn new(id: &str, collection: &str) -> Self {
    Self {
      id: id.to_string(),
      collection: collection.to_string(),
    }
  }
}

pub fn cascade_value(entity_id: &str) -> serde_json::Value {
  serde_json::Value::String(entity_id.to_string())
}

#[allow(dead_code)]
pub fn insert_cascade_id(
  deleted_ids: &mut HashSet<String>,
  to_process: &mut Vec<CascadeEntityRef>,
  id: &str,
  collection: &str,
) {
  if !deleted_ids.contains(id) {
    deleted_ids.insert(id.to_string());
    to_process.push(CascadeEntityRef::new(id, collection));
  }
}
