use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use serde_json::Value;
use std::collections::HashMap;

use super::super::helpers::{apply_filter, filter_not_deleted, inject_collection};
use super::super::types::RelationDef;

pub async fn load<P: DatabaseProvider>(
  provider: P,
  docs: &mut [Value],
  relation: &RelationDef,
  filter_deleted: bool,
) -> OrmResult<Vec<Value>> {
  let join_field = relation.join_field.as_deref().unwrap_or("ids");

  let all_ids: Vec<String> = {
    let mut ids = Vec::new();
    for doc in docs.iter() {
      if let Some(arr) = doc.get(join_field).and_then(|v| v.as_array()) {
        for item in arr {
          if let Some(id) = item.as_str() {
            ids.push(id.to_string());
          }
        }
      }
    }
    ids
  };

  if all_ids.is_empty() {
    return Ok(docs.to_vec());
  }

  let base_filter = Filter::In(
    "id".to_string(),
    all_ids.iter().map(|s| Value::String(s.clone())).collect(),
  );

  let filter = if filter_deleted {
    apply_filter(Some(&base_filter))
  } else {
    Some(base_filter)
  };

  let mut related_docs = provider
    .find_many(
      &relation.target_collection,
      filter.as_ref(),
      None,
      None,
      None,
      true,
    )
    .await?;

  related_docs = inject_collection(related_docs, &relation.target_collection);

  if filter_deleted {
    related_docs = filter_not_deleted(related_docs);
  }

  let related_map: HashMap<String, Value> = related_docs
    .into_iter()
    .filter_map(|d| {
      d.clone()
        .get("id")
        .and_then(|id| id.as_str())
        .map(|id| (id.to_string(), d))
    })
    .collect();

  for doc in docs.iter_mut() {
    if let Some(obj) = doc.as_object_mut() {
      if let Some(arr) = obj.get(join_field).and_then(|v| v.as_array()) {
        let mut resolved: Vec<Value> = Vec::new();
        for item in arr.iter() {
          if let Some(id) = item.as_str() {
            if let Some(mut related) = related_map.get(id).cloned() {
              if let Some(rel_obj) = related.as_object_mut() {
                rel_obj.insert("_collection".to_string(), Value::String(relation.target_collection.clone()));
              }
              resolved.push(related);
            }
          }
        }
        obj.insert(relation.name.clone(), Value::Array(resolved));
      }
    }
  }

  Ok(docs.to_vec())
}
