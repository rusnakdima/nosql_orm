use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use serde_json::Value;

use super::super::registry::{get_registered_collection_relations, get_relation_def};
use super::super::types::RelationType;
use super::RelationLoader;

pub async fn load_recursive<P: DatabaseProvider>(
  provider: P,
  mut docs: Vec<Value>,
  path_segments: &[&str],
  base_collection: &str,
  filter_deleted: bool,
  ancestors: &mut std::collections::HashSet<String>,
  loader: &RelationLoader<P>,
) -> OrmResult<Vec<Value>> {
  if path_segments.is_empty() || docs.is_empty() {
    return Ok(docs);
  }

  let current_collection = base_collection.to_string();

  let first_segment = path_segments[0];

  let rel_def = get_relation_def(&current_collection, first_segment).ok_or_else(|| {
    let available = get_registered_collection_relations(&current_collection)
      .map(|rels| {
        rels
          .iter()
          .map(|r| r.name.as_str())
          .collect::<Vec<_>>()
          .join(", ")
      })
      .unwrap_or_else(|| "none".to_string());
    OrmError::InvalidQuery(format!(
      "Unknown relation '{}' on collection '{}'. Available: [{}]",
      first_segment, current_collection, available
    ))
  })?;

  let children_already_loaded = docs.iter().any(|doc| {
    if let Some(arr) = doc.get(first_segment).and_then(|v| v.as_array()) {
      if arr.iter().any(|child| child.get("_collection").is_some()) {
        return true;
      }
    }
    if doc
      .get(first_segment)
      .and_then(|v| v.as_object())
      .is_some_and(|obj| obj.get("_collection").is_some())
    {
      return true;
    }
    false
  });

  if !children_already_loaded {
    docs = loader.load_many(docs, &rel_def, filter_deleted).await?;
  }

  if path_segments.len() > 1 {
    docs = loader
      .load_all_relations(docs, &current_collection, filter_deleted, ancestors)
      .await?;
  }

  if path_segments.len() == 1 {
    return Ok(docs);
  }

  let target_collection = rel_def.target_collection.clone();
  let remaining_segments = &path_segments[1..];

  let mut all_children: Vec<Value> = Vec::new();
  let mut parent_child_pairs: Vec<(String, Value)> = Vec::new();

  for doc in &docs {
    let parent_id = doc
      .get("id")
      .and_then(|v| v.as_str())
      .unwrap_or("")
      .to_string();
    if rel_def.relation_type == RelationType::ManyToOne
      || rel_def.relation_type == RelationType::OneToOne
    {
      if let Some(rel_obj) = doc.get(first_segment) {
        if !rel_obj.is_null() {
          let mut child_with_meta = rel_obj.clone();
          if let Some(obj) = child_with_meta.as_object_mut() {
            obj.insert(
              "_collection".to_string(),
              Value::String(target_collection.clone()),
            );
          }
          all_children.push(child_with_meta.clone());
          parent_child_pairs.push((parent_id.clone(), child_with_meta));
        }
      }
    } else if let Some(arr) = doc.get(first_segment).and_then(|v| v.as_array()) {
      for child in arr {
        let mut child_with_meta = child.clone();
        if let Some(obj) = child_with_meta.as_object_mut() {
          obj.insert(
            "_collection".to_string(),
            Value::String(target_collection.clone()),
          );
        }
        all_children.push(child_with_meta.clone());
        parent_child_pairs.push((parent_id.clone(), child_with_meta));
      }
    }
  }

  if all_children.is_empty() {
    return Ok(docs);
  }

  let enriched_children = Box::pin(load_recursive(
    provider.clone(),
    all_children,
    remaining_segments,
    &target_collection,
    filter_deleted,
    ancestors,
    loader,
  ))
  .await?;

  let mut children_by_parent: std::collections::HashMap<String, Vec<Value>> =
    std::collections::HashMap::new();
  for (i, child) in enriched_children.into_iter().enumerate() {
    if let Some((parent_id, _)) = parent_child_pairs.get(i) {
      children_by_parent
        .entry(parent_id.clone())
        .or_default()
        .push(child);
    }
  }

  for doc in docs.iter_mut() {
    if let Some(obj) = doc.as_object_mut() {
      if let Some(parent_id) = obj.get("id").and_then(|v| v.as_str()) {
        if let Some(enriched) = children_by_parent.get(parent_id) {
          match rel_def.relation_type {
            RelationType::ManyToOne | RelationType::OneToOne => {
              obj.insert(
                first_segment.to_string(),
                enriched
                  .iter()
                  .next()
                  .cloned()
                  .unwrap_or(serde_json::Value::Null),
              );
            }
            RelationType::OneToMany | RelationType::ManyToMany => {
              obj.insert(
                first_segment.to_string(),
                serde_json::Value::Array(enriched.clone()),
              );
            }
          }
        }
      }
    }
  }

  Ok(docs)
}
