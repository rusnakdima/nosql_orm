use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use serde_json::Value;

use super::super::registry::get_collection_relations;
use super::super::types::RelationDef;
use super::RelationLoader;

pub async fn load<P: DatabaseProvider>(
  _provider: P,
  docs: Vec<Value>,
  relation: &RelationDef,
  visited: &mut std::collections::HashSet<String>,
  loader: &RelationLoader<P>,
) -> OrmResult<Vec<Value>> {
  let target = relation.target_collection.clone();

  if visited.contains(&target) {
    return Ok(docs);
  }

  visited.insert(target.clone());

  let mut result_docs = loader.load_many(docs, relation, true).await?;

  if let Some(child_relations) = get_collection_relations(&target) {
    for child_rel in child_relations {
      let child_target = child_rel.target_collection.clone();

      if visited.contains(&child_target) {
        continue;
      }

      visited.insert(child_target.clone());

      let child_docs: Vec<Value> = result_docs
        .iter()
        .filter_map(|d| d.get(&relation.name).and_then(|v| v.as_array()).cloned())
        .flatten()
        .collect();

      if child_docs.is_empty() {
        visited.remove(&child_target);
        continue;
      }

      let enriched = loader.load_many(child_docs, &child_rel, true).await?;

      let mut to_process = vec![(
        enriched.clone(),
        child_target.clone(),
        child_rel.name.clone(),
      )];

      while let Some((current_docs, current_target, current_rel_name)) = to_process.pop() {
        if let Some(grandchild_relations) = get_collection_relations(&current_target) {
          for grandchild_rel in grandchild_relations {
            let grandchild_target = grandchild_rel.target_collection.clone();

            if visited.contains(&grandchild_target) {
              continue;
            }

            visited.insert(grandchild_target.clone());

            let grandchild_docs: Vec<Value> = current_docs
              .iter()
              .filter_map(|d| d.get(&current_rel_name).and_then(|v| v.as_array()).cloned())
              .flatten()
              .collect();

            if !grandchild_docs.is_empty() {
              let grandchild_enriched = loader
                .load_many(grandchild_docs, &grandchild_rel, true)
                .await?;

              if let Some(gc_relations) = get_collection_relations(&grandchild_target) {
                for gc_rel in gc_relations {
                  to_process.push((
                    grandchild_enriched.clone(),
                    gc_rel.target_collection.clone(),
                    gc_rel.name.clone(),
                  ));
                }
              }
            }

            visited.remove(&grandchild_target);
          }
        }
      }

      let child_map: std::collections::HashMap<String, Value> = enriched
        .into_iter()
        .filter_map(|d| {
          d.clone()
            .get("id")
            .and_then(|v| v.as_str())
            .map(|id| (id.to_string(), d))
        })
        .collect();

      for doc in result_docs.iter_mut() {
        if let Some(obj) = doc.as_object_mut() {
          if let Some(arr) = obj.get_mut(&relation.name) {
            if let Some(arr_mut) = arr.as_array_mut() {
              for item in arr_mut.iter_mut() {
                if let Some(obj_item) = item.as_object_mut() {
                  if let Some(item_id) = obj_item.get("id").and_then(|v| v.as_str()) {
                    if let Some(enriched_child) = child_map.get(item_id) {
                      *item = enriched_child.clone();
                    }
                  }
                }
              }
            }
          }
        }
      }

      visited.remove(&child_target);
    }
  }

  visited.remove(&target);

  Ok(result_docs)
}
