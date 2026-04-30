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
  let source_key = "id";

  let parent_ids: Vec<String> = docs
    .iter()
    .filter_map(|d| d.get(source_key).and_then(|v| v.as_str()).map(String::from))
    .collect();

  if parent_ids.is_empty() {
    return Ok(docs.to_vec());
  }

  let base_filter = Filter::In(
    relation.foreign_key.clone(),
    parent_ids
      .iter()
      .map(|s| Value::String(s.clone()))
      .collect(),
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

  let grouped: HashMap<String, Vec<Value>> = {
    let mut map = HashMap::new();
    for mut rel_doc in related_docs {
      if let Some(obj) = rel_doc.as_object_mut() {
        obj.insert(
          "_collection".to_string(),
          Value::String(relation.target_collection.clone()),
        );
      }
      if let Some(fk_val) = rel_doc.get(&relation.foreign_key).and_then(|v| v.as_str()) {
        map
          .entry(fk_val.to_string())
          .or_insert_with(Vec::new)
          .push(rel_doc);
      }
    }
    map
  };

  for doc in docs.iter_mut() {
    if let Some(obj) = doc.as_object_mut() {
      if let Some(parent_id) = obj.get(source_key).and_then(|v| v.as_str()) {
        let mut related = grouped.get(parent_id).cloned().unwrap_or_default();
        for rel_doc in &mut related {
          if let Some(rel_obj) = rel_doc.as_object_mut() {
            if rel_obj.get("_collection").is_none() {
              rel_obj.insert(
                "_collection".to_string(),
                Value::String(relation.target_collection.clone()),
              );
            }
          }
        }
        obj.insert(relation.name.clone(), Value::Array(related));
      }
    }
  }

  Ok(docs.to_vec())
}
