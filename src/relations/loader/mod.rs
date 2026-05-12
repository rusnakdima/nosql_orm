use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use serde_json::Value;
use std::collections::HashMap;

use super::registry::{
  get_collection_relations, get_registered_collection_relations, get_relation_def,
};
use super::types::{RelationDef, RelationType, RelationValue};

mod cascade;
mod many_to_many;
mod many_to_one;
mod nested;
mod one_to_many;
mod recursive;

pub struct RelationLoader<P: DatabaseProvider> {
  provider: P,
}

impl<P: DatabaseProvider> RelationLoader<P> {
  pub fn new(provider: P) -> Self {
    Self { provider }
  }

  pub async fn load_many(
    &self,
    mut docs: Vec<Value>,
    relation: &RelationDef,
    filter_deleted: bool,
  ) -> OrmResult<Vec<Value>> {
    let result = match relation.relation_type {
      RelationType::ManyToOne | RelationType::OneToOne => {
        many_to_one::load(self.provider.clone(), &mut docs, relation, filter_deleted).await
      }
      RelationType::OneToMany => {
        one_to_many::load(self.provider.clone(), &mut docs, relation, filter_deleted).await
      }
      RelationType::ManyToMany => {
        many_to_many::load(self.provider.clone(), &mut docs, relation, filter_deleted).await
      }
    };

    result
  }

  pub async fn load(
    &self,
    doc: &Value,
    relations: &[RelationDef],
    filter_deleted: bool,
  ) -> OrmResult<HashMap<String, RelationValue>> {
    let mut current_doc = doc.clone();
    let mut loaded = HashMap::new();

    for rel in relations {
      let result = self
        .load_many(vec![current_doc.clone()], rel, filter_deleted)
        .await?;
      if let Some(updated) = result.first() {
        if let Some(rel_val) = updated.get(&rel.name) {
          match rel.relation_type {
            RelationType::ManyToOne | RelationType::OneToOne => {
              loaded.insert(
                rel.name.clone(),
                RelationValue::Single(Some(rel_val.clone())),
              );
            }
            RelationType::OneToMany | RelationType::ManyToMany => {
              if let Some(arr) = rel_val.as_array() {
                loaded.insert(rel.name.clone(), RelationValue::Many(arr.clone()));
              }
            }
          }
        }
        current_doc = updated.clone();
      }
    }

    Ok(loaded)
  }

  pub async fn load_relation(&self, doc: &Value, relation: &RelationDef) -> OrmResult<Value> {
    match relation.relation_type {
      RelationType::ManyToOne | RelationType::OneToOne => {
        let id_val = doc.get(&relation.local_key).and_then(|v| v.as_str());
        match id_val {
          None => Ok(doc.clone()),
          Some(id) => {
            if let Some(found) = self
              .provider
              .find_by_id(&relation.target_collection, id)
              .await?
            {
              let mut result = doc.clone();
              if let Some(obj) = result.as_object_mut() {
                obj.insert(relation.name.clone(), found);
              }
              Ok(result)
            } else {
              Ok(doc.clone())
            }
          }
        }
      }
      RelationType::OneToMany => {
        let local_id = doc.get(&relation.local_key).and_then(|v| v.as_str());
        match local_id {
          None => Ok(doc.clone()),
          Some(id) => {
            let filter = Filter::Eq(relation.foreign_key.clone(), Value::String(id.to_string()));
            let docs = self
              .provider
              .find_many(
                &relation.target_collection,
                Some(&filter),
                None,
                None,
                None,
                true,
              )
              .await?;
            let mut result = doc.clone();
            if let Some(obj) = result.as_object_mut() {
              obj.insert(relation.name.clone(), Value::Array(docs));
            }
            Ok(result)
          }
        }
      }
      RelationType::ManyToMany => {
        let join_field = relation.join_field.as_deref().unwrap_or("ids");
        let ids: Vec<&str> = doc
          .get(join_field)
          .and_then(|v| v.as_array())
          .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
          .unwrap_or_default();

        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
          if let Some(found) = self
            .provider
            .find_by_id(&relation.target_collection, id)
            .await?
          {
            results.push(found);
          }
        }
        let mut result = doc.clone();
        if let Some(obj) = result.as_object_mut() {
          obj.insert(relation.name.clone(), Value::Array(results));
        }
        Ok(result)
      }
    }
  }

  pub async fn load_relation_recursive(
    &self,
    docs: Vec<Value>,
    relation: &RelationDef,
    visited: &mut std::collections::HashSet<String>,
  ) -> OrmResult<Vec<Value>> {
    recursive::load(self.provider.clone(), docs, relation, visited, self).await
  }

  pub async fn load_all_relations(
    &self,
    mut docs: Vec<Value>,
    collection: &str,
    filter_deleted: bool,
    ancestors: &mut std::collections::HashSet<String>,
  ) -> OrmResult<Vec<Value>> {
    if docs.is_empty() {
      return Ok(docs);
    }

    if ancestors.contains(collection) {
      return Ok(docs);
    }
    ancestors.insert(collection.to_string());

    let relations = match get_collection_relations(collection) {
      Some(r) => r,
      None => {
        ancestors.remove(collection);
        return Ok(docs);
      }
    };

    for rel_def in relations {
      docs = self.load_many(docs, &rel_def, filter_deleted).await?;

      {
        let segment = rel_def.name.as_str();
        let target_collection = rel_def.target_collection.clone();

        if ancestors.contains(&target_collection) {
          continue;
        }

        let mut all_children: Vec<Value> = Vec::new();
        let mut child_parent_map: Vec<(Value, Value)> = Vec::new();

        for doc in &docs {
          if rel_def.relation_type == RelationType::ManyToOne {
            if let Some(rel_obj) = doc.get(segment) {
              if !rel_obj.is_null() {
                let mut child_with_meta = rel_obj.clone();
                if let Some(obj) = child_with_meta.as_object_mut() {
                  obj.insert(
                    "_collection".to_string(),
                    Value::String(target_collection.clone()),
                  );
                }
                all_children.push(child_with_meta.clone());
                child_parent_map.push((doc.clone(), child_with_meta));
              }
            }
          } else if let Some(arr) = doc.get(segment).and_then(|v| v.as_array()) {
            for child in arr {
              let mut child_with_meta = child.clone();
              if let Some(obj) = child_with_meta.as_object_mut() {
                obj.insert(
                  "_collection".to_string(),
                  Value::String(target_collection.clone()),
                );
              }
              all_children.push(child_with_meta.clone());
              child_parent_map.push((doc.clone(), child_with_meta));
            }
          }
        }

        if !all_children.is_empty() {
          let enriched_children = Box::pin(self.load_all_relations(
            all_children,
            &target_collection,
            filter_deleted,
            ancestors,
          ))
          .await?;

          let mut children_by_parent: std::collections::HashMap<String, Vec<Value>> =
            std::collections::HashMap::new();

          for (i, child) in enriched_children.into_iter().enumerate() {
            if let Some(parent_doc) = child_parent_map.get(i) {
              let parent_id = parent_doc
                .0
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
              children_by_parent
                .entry(parent_id.to_string())
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
                        segment.to_string(),
                        enriched
                          .iter()
                          .next()
                          .cloned()
                          .unwrap_or(serde_json::Value::Null),
                      );
                    }
                    RelationType::OneToMany | RelationType::ManyToMany => {
                      obj.insert(
                        segment.to_string(),
                        serde_json::Value::Array(enriched.clone()),
                      );
                    }
                  }
                }
              }
            }
          }
        }
      }
    }

    ancestors.remove(collection);
    Ok(docs)
  }

  pub async fn load_nested_recursive(
    &self,
    docs: Vec<Value>,
    path_segments: &[&str],
    base_collection: &str,
    filter_deleted: bool,
    ancestors: &mut std::collections::HashSet<String>,
  ) -> OrmResult<Vec<Value>> {
    nested::load_recursive(
      self.provider.clone(),
      docs,
      path_segments,
      base_collection,
      filter_deleted,
      ancestors,
      self,
    )
    .await
  }

  pub async fn load_nested(
    &self,
    docs: Vec<Value>,
    path_segments: &[&str],
    base_collection: &str,
    filter_deleted: bool,
  ) -> OrmResult<Vec<Value>> {
    let mut ancestors = std::collections::HashSet::new();
    self
      .load_nested_recursive(
        docs,
        path_segments,
        base_collection,
        filter_deleted,
        &mut ancestors,
      )
      .await
  }

  pub async fn load_nested_relations(
    &self,
    docs: Vec<Value>,
    path_segments: &[&str],
    parent_relation: &RelationDef,
    filter_deleted: bool,
  ) -> OrmResult<Vec<Value>> {
    if path_segments.is_empty() || docs.is_empty() {
      return Ok(docs);
    }

    let target_collection = parent_relation.target_collection.clone();

    let mut docs_with_meta = docs;
    for doc in &mut docs_with_meta {
      if let Some(obj) = doc.as_object_mut() {
        obj.insert(
          "_collection".to_string(),
          Value::String(target_collection.clone()),
        );
      }
    }

    let mut ancestors = std::collections::HashSet::new();
    self
      .load_nested_recursive(
        docs_with_meta,
        path_segments,
        &target_collection,
        filter_deleted,
        &mut ancestors,
      )
      .await
  }

  fn find_child_relation(
    &self,
    parent_relation: &RelationDef,
    segment: &str,
  ) -> OrmResult<RelationDef> {
    let target_collection = &parent_relation.target_collection;

    if let Some(child_relations) = get_collection_relations(target_collection) {
      if let Some(rel) = child_relations.iter().find(|r| r.name.as_str() == segment) {
        return Ok(rel.clone());
      }
    }

    let relations_from_def = Self::get_relations_for_collection(target_collection);
    if let Some(rel) = relations_from_def
      .iter()
      .find(|r| r.name.as_str() == segment)
    {
      return Ok(rel.clone());
    }

    Err(OrmError::InvalidQuery(format!(
      "Unknown relation '{}' on collection '{}'. Available relations: {:?}",
      segment,
      target_collection,
      relations_from_def
        .iter()
        .map(|r| r.name.as_str())
        .collect::<Vec<_>>()
    )))
  }

  pub fn get_relations_for_collection(collection: &str) -> Vec<RelationDef> {
    get_collection_relations(collection).unwrap_or_default()
  }

  fn get_relation_def_for_path(&self, docs: &[Value], segment: &str) -> OrmResult<RelationDef> {
    if docs.is_empty() {
      return Err(OrmError::Internal("No documents provided".into()));
    }

    let collection = docs[0]
      .get("_collection")
      .and_then(|v| v.as_str())
      .unwrap_or("");

    get_relation_def(collection, segment).ok_or_else(|| {
      let available = get_registered_collection_relations(collection);
      OrmError::Internal(
        format!(
          "Unknown relation '{}' on collection '{}'. Available: {:?}",
          segment, collection, available
        )
        .into(),
      )
    })
  }

  pub async fn load_relations_on_docs(
    &self,
    mut docs: Vec<Value>,
    table: &str,
    paths: &[&str],
    filter_deleted: bool,
  ) -> OrmResult<Vec<Value>> {
    for path in paths {
      let segments: Vec<&str> = path.split('.').collect();
      if segments.is_empty() {
        continue;
      }

      docs = self
        .load_nested(docs, &segments, table, filter_deleted)
        .await?;
    }

    Ok(docs)
  }

  pub async fn load_cascade_for_entity(
    &self,
    entity_doc: &Value,
    table: &str,
    path: &str,
    filter_deleted: bool,
  ) -> OrmResult<HashMap<String, RelationValue>> {
    cascade::load(
      self.provider.clone(),
      entity_doc,
      table,
      path,
      filter_deleted,
      self,
    )
    .await
  }
}
