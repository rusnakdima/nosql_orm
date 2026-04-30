use std::collections::HashSet;

#[derive(Clone, Copy)]
pub enum CascadeAction {
  SoftDelete,
  Restore,
}

pub fn cascade_value(entity_id: &str) -> serde_json::Value {
  serde_json::Value::String(entity_id.to_string())
}

pub fn insert_cascade_id(
  deleted_ids: &mut HashSet<String>,
  to_process: &mut Vec<String>,
  id: &str,
) {
  if !deleted_ids.contains(id) {
    deleted_ids.insert(id.to_string());
    to_process.push(id.to_string());
  }
}
