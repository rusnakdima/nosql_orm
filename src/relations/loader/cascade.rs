use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use serde_json::Value;
use std::collections::HashMap;

use super::super::registry::get_relation_def;
use super::super::types::RelationValue;
use super::RelationLoader;

pub async fn load<P: DatabaseProvider>(
  _provider: P,
  entity_doc: &Value,
  table: &str,
  path: &str,
  filter_deleted: bool,
  loader: &RelationLoader<P>,
) -> OrmResult<HashMap<String, RelationValue>> {
  let mut results = HashMap::new();
  let segments: Vec<&str> = path.split('.').collect();

  if segments.is_empty() {
    return Ok(results);
  }

  let first = segments[0];
  let rel_def = get_relation_def(table, first).ok_or_else(|| {
    OrmError::InvalidQuery(format!("Unknown relation '{}' on '{}'", first, table))
  })?;

  let mut doc_with_collection = entity_doc.clone();
  if let Some(obj) = doc_with_collection.as_object_mut() {
    obj.insert("_collection".to_string(), Value::String(table.to_string()));
  }

  let loaded = loader
    .load(
      &doc_with_collection,
      std::slice::from_ref(&rel_def),
      filter_deleted,
    )
    .await?;

  if let Some(value) = loaded.get(first) {
    results.insert(first.to_string(), value.clone());

    if segments.len() > 1 {
      let related_docs: Vec<Value> = match value {
        RelationValue::Single(v) => v.as_ref().map(|v| vec![v.clone()]).unwrap_or_default(),
        RelationValue::Many(arr) => arr.clone(),
      };

      if !related_docs.is_empty() {
        let mut docs_with_meta = related_docs;
        for d in &mut docs_with_meta {
          if let Some(obj) = d.as_object_mut() {
            obj.insert(
              "_collection".to_string(),
              Value::String(rel_def.target_collection.clone()),
            );
          }
        }

        let nested_docs = loader
          .load_nested(
            docs_with_meta,
            &segments[1..],
            &rel_def.target_collection,
            filter_deleted,
          )
          .await?;

        let mut level_docs: Vec<Vec<Value>> = vec![];

        for seg in &segments {
          let seg_docs: Vec<Value> = nested_docs
            .iter()
            .filter_map(|d| d.get(*seg as &str).and_then(|v| v.as_array()))
            .flatten()
            .cloned()
            .collect();

          level_docs.push(seg_docs.clone());
        }

        for (i, _) in segments.iter().enumerate().skip(1) {
          let mut prefix = String::new();
          for (j, seg) in segments.iter().enumerate().take(i + 1) {
            if j > 0 {
              prefix.push('.');
            }
            prefix.push_str(seg);
          }

          if i < level_docs.len() && !level_docs[i].is_empty() {
            results.insert(prefix, RelationValue::Many(level_docs[i].clone()));
          }
        }
      }
    }
  }

  Ok(results)
}
