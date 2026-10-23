use crate::error::{OrmError, OrmResult};
use crate::provider::Storage;
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

pub struct RelationLoader<P: Storage> {
  provider: P,
}

#[allow(dead_code)]
impl<P: Storage> RelationLoader<P> {
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
          if rel_def.relation_type == RelationType::ManyToOne
            || rel_def.relation_type == RelationType::OneToOne
          {
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
      OrmError::Internal(format!(
        "Unknown relation '{}' on collection '{}'. Available: {:?}",
        segment, collection, available
      ))
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

  pub fn provider(&self) -> &P {
    &self.provider
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::nosql_index::NosqlIndex;
  use crate::provider::{
    AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth, IndexInfo,
    RawResult, SchemaIntrospection, Storage, TransactionControl, TransactionId,
  };
  use crate::query::Filter;
  use async_trait::async_trait;
  use parking_lot::RwLock;
  use serde_json::Value;
  use std::collections::HashMap;
  use tempfile::tempdir;

  #[derive(Clone)]
  struct MockStorage {
    data: std::sync::Arc<RwLock<HashMap<String, Vec<Value>>>>,
  }

  impl MockStorage {
    fn new() -> Self {
      Self {
        data: std::sync::Arc::new(RwLock::new(HashMap::new())),
      }
    }

    fn with_data(self, collection: &str, docs: Vec<Value>) -> Self {
      self.data.write().insert(collection.to_string(), docs);
      self
    }
  }

  impl Default for MockStorage {
    fn default() -> Self {
      Self::new()
    }
  }

  #[async_trait]
  impl Storage for MockStorage {
    async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
      self
        .data
        .write()
        .entry(collection.to_string())
        .or_default()
        .push(doc.clone());
      Ok(doc)
    }

    async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
      let data = self.data.read();
      let docs = data.get(collection).cloned().unwrap_or_default();
      let found = docs
        .iter()
        .find(|d| d.get("id").and_then(|v| v.as_str()) == Some(id));
      Ok(found.cloned())
    }

    async fn find_many(
      &self,
      collection: &str,
      filter: Option<&Filter>,
      _skip: Option<u64>,
      _limit: Option<u64>,
      _sort_by: Option<&str>,
      _sort_asc: bool,
    ) -> OrmResult<Vec<Value>> {
      let data = self.data.read();
      let docs: Vec<Value> = data.get(collection).cloned().unwrap_or_default();
      drop(data);
      let filtered = match filter {
        Some(Filter::Eq(key, val)) => docs
          .into_iter()
          .filter(|d| d.get(key) == Some(val))
          .collect(),
        _ => docs,
      };
      Ok(filtered)
    }

    async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
      let mut data = self.data.write();
      if let Some(existing) = data
        .get_mut(collection)
        .unwrap()
        .iter_mut()
        .find(|d| d.get("id").and_then(|v| v.as_str()) == Some(id))
      {
        *existing = doc.clone();
      }
      Ok(doc)
    }

    async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
      let mut data = self.data.write();
      if let Some(existing) = data
        .get_mut(collection)
        .unwrap()
        .iter_mut()
        .find(|d| d.get("id").and_then(|v| v.as_str()) == Some(id))
      {
        if let (Some(obj), Some(patch_obj)) = (existing.as_object_mut(), patch.as_object()) {
          obj.extend(patch_obj.clone());
        }
        return Ok(existing.clone());
      }
      Ok(Value::Null)
    }

    async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
      let mut data = self.data.write();
      let docs = data.get_mut(collection).unwrap();
      let len_before = docs.len();
      docs.retain(|d| d.get("id").and_then(|v| v.as_str()) != Some(id));
      Ok(docs.len() < len_before)
    }

    async fn delete_many(&self, _collection: &str, _filter: Option<Filter>) -> OrmResult<usize> {
      Ok(0)
    }

    async fn update_many(
      &self,
      _collection: &str,
      _filter: Option<Filter>,
      _updates: Value,
    ) -> OrmResult<usize> {
      Ok(0)
    }

    async fn count(&self, collection: &str, _filter: Option<&Filter>) -> OrmResult<u64> {
      let data = self.data.read();
      Ok(data.get(collection).map(|d| d.len()).unwrap_or(0) as u64)
    }

    async fn exists(&self, collection: &str, id: &str) -> OrmResult<bool> {
      self
        .find_by_id(collection, id)
        .await
        .map(|opt| opt.is_some())
    }

    async fn create_index(&self, _collection: &str, _index: &NosqlIndex) -> OrmResult<()> {
      Ok(())
    }

    async fn drop_index(&self, _collection: &str, _index_name: &str) -> OrmResult<()> {
      Ok(())
    }

    async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
      Ok(vec![])
    }

    async fn health_check(&self) -> OrmResult<bool> {
      Ok(true)
    }

    async fn aggregate(&self, _collection: &str, _pipeline: Vec<Value>) -> OrmResult<Vec<Value>> {
      Err(OrmError::NotSupported(
        "aggregate not implemented".to_string(),
      ))
    }

    async fn insert_many(&self, collection: &str, docs: Vec<Value>) -> OrmResult<usize> {
      let mut count = 0;
      for doc in docs {
        self.insert(collection, doc).await?;
        count += 1;
      }
      Ok(count)
    }
  }

  #[async_trait]
  impl AdminCommands for MockStorage {
    async fn execute_raw(&self, _sql: &str, _params: Vec<Value>) -> OrmResult<RawResult> {
      Ok(RawResult {
        columns: vec![],
        rows: vec![],
        affected_rows: 0,
        last_insert_id: None,
      })
    }
    async fn create_collection(
      &self,
      _name: &str,
      _schema: Option<CollectionSchema>,
    ) -> OrmResult<()> {
      Ok(())
    }
    async fn drop_collection(&self, _name: &str) -> OrmResult<()> {
      Ok(())
    }
    async fn rename_collection(&self, _old_name: &str, _new_name: &str) -> OrmResult<()> {
      Ok(())
    }
    async fn get_server_version(&self) -> OrmResult<String> {
      Ok("mock".to_string())
    }
    async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
      Ok(ConnectionHealth {
        healthy: true,
        latency_ms: Some(0),
        server_version: None,
        connected_at: None,
        pool_stats: None,
      })
    }
  }

  #[async_trait]
  impl SchemaIntrospection for MockStorage {
    async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
      Ok(vec![])
    }
    async fn describe_collection(&self, _collection: &str) -> OrmResult<CollectionSchema> {
      Ok(CollectionSchema {
        name: String::new(),
        fields: HashMap::new(),
        indexes: vec![],
        options: crate::admin_types::CollectionOptions::default(),
      })
    }
    async fn get_collection_stats(&self, _collection: &str) -> OrmResult<CollectionStats> {
      Ok(CollectionStats {
        name: String::new(),
        document_count: 0,
        size_bytes: 0,
        storage_size_bytes: 0,
        index_count: 0,
        index_size_bytes: 0,
        average_document_size: 0,
      })
    }
    async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
      Ok(vec![])
    }
    async fn get_database_name(&self) -> OrmResult<String> {
      Ok(String::new())
    }
    async fn list_databases(&self) -> OrmResult<Vec<String>> {
      Ok(vec![])
    }
  }

  #[async_trait]
  impl TransactionControl for MockStorage {
    async fn begin_transaction(&self) -> OrmResult<TransactionId> {
      Ok(TransactionId("mock".to_string()))
    }
    async fn commit_transaction(&self, _id: TransactionId) -> OrmResult<()> {
      Ok(())
    }
    async fn rollback_transaction(&self, _id: TransactionId) -> OrmResult<()> {
      Ok(())
    }
    async fn is_transaction_active(&self) -> OrmResult<bool> {
      Ok(false)
    }
  }

  #[tokio::test]
  async fn test_load_relations_empty() -> OrmResult<()> {
    let _temp = tempdir()?;
    let storage = MockStorage::new();
    let loader = RelationLoader::new(storage);

    let relation = RelationDef::many_to_one("author", "users", "author_id");
    let docs: Vec<Value> = vec![];

    let result = loader.load_many(docs, &relation, false).await?;
    assert!(result.is_empty());

    Ok(())
  }

  #[tokio::test]
  async fn test_load_relations_single() -> OrmResult<()> {
    let storage = MockStorage::new()
      .with_data(
        "users",
        vec![
          serde_json::json!({"id": "u1", "name": "Alice"}),
          serde_json::json!({"id": "u2", "name": "Bob"}),
        ],
      )
      .with_data(
        "posts",
        vec![serde_json::json!({"id": "p1", "title": "Hello", "author_id": "u1"})],
      );

    let loader = RelationLoader::new(storage);

    let relation = RelationDef::many_to_one("author", "users", "author_id");
    let docs = vec![serde_json::json!({"id": "p1", "title": "Hello", "author_id": "u1"})];

    let result = loader.load_many(docs, &relation, false).await?;
    assert_eq!(result.len(), 1);
    let author = result[0]
      .get("author")
      .expect("author relation should be loaded");
    assert_eq!(author.get("id").and_then(|v| v.as_str()), Some("u1"));
    assert_eq!(author.get("name").and_then(|v| v.as_str()), Some("Alice"));

    Ok(())
  }

  #[tokio::test]
  async fn test_load_relations_nested() -> OrmResult<()> {
    let storage = MockStorage::new()
      .with_data(
        "users",
        vec![
          serde_json::json!({"id": "u1", "name": "Alice"}),
          serde_json::json!({"id": "u2", "name": "Bob"}),
        ],
      )
      .with_data(
        "posts",
        vec![
          serde_json::json!({"id": "p1", "title": "Hello", "author_id": "u1"}),
          serde_json::json!({"id": "p2", "title": "World", "author_id": "u1"}),
        ],
      )
      .with_data(
        "comments",
        vec![
          serde_json::json!({"id": "c1", "text": "Great!", "post_id": "p1"}),
          serde_json::json!({"id": "c2", "text": "Nice!", "post_id": "p1"}),
          serde_json::json!({"id": "c3", "text": "Cool!", "post_id": "p2"}),
        ],
      );

    let loader = RelationLoader::new(storage);

    // Load comments -> posts relation (ManyToOne via foreign_key)
    let posts_relation = RelationDef::many_to_one("post", "posts", "post_id");
    let comments = vec![
      serde_json::json!({"id": "c1", "text": "Great!", "post_id": "p1"}),
      serde_json::json!({"id": "c2", "text": "Nice!", "post_id": "p1"}),
      serde_json::json!({"id": "c3", "text": "Cool!", "post_id": "p2"}),
    ];

    let result = loader.load_many(comments, &posts_relation, false).await?;
    assert_eq!(result.len(), 3);

    // Verify first comment's post is loaded
    let c1_post = result[0]
      .get("post")
      .expect("post relation should be loaded");
    assert_eq!(c1_post.get("id").and_then(|v| v.as_str()), Some("p1"));
    assert_eq!(c1_post.get("title").and_then(|v| v.as_str()), Some("Hello"));

    // Verify author nested relation via load_all_relations
    let comments_with_meta: Vec<Value> = result
      .iter()
      .map(|c| {
        let mut doc = c.clone();
        if let Some(obj) = doc.as_object_mut() {
          obj.insert(
            "_collection".to_string(),
            Value::String("comments".to_string()),
          );
        }
        doc
      })
      .collect();

    let mut ancestors = std::collections::HashSet::new();
    let posts_rel = RelationDef::many_to_one("post", "posts", "post_id");
    let users_rel = RelationDef::many_to_one("author", "users", "author_id");

    // Register relations for the nested load test
    crate::relations::registry::register_collection_relations("comments", vec![posts_rel]);
    crate::relations::registry::register_collection_relations("posts", vec![users_rel]);

    let final_result = loader
      .load_all_relations(comments_with_meta, "comments", false, &mut ancestors)
      .await?;

    assert_eq!(final_result.len(), 3);

    // Clean up registry
    crate::relations::registry::clear_relation_registry();
    crate::relations::registry::clear_registered_collections();

    Ok(())
  }

  #[tokio::test]
  async fn test_save_and_reload() -> OrmResult<()> {
    let temp = tempdir().expect("failed to create temp dir");

    let storage = MockStorage::new()
      .with_data(
        "products",
        vec![
          serde_json::json!({"id": "prod1", "name": "Widget", "category_id": "cat1"}),
          serde_json::json!({"id": "prod2", "name": "Gadget", "category_id": "cat1"}),
        ],
      )
      .with_data(
        "categories",
        vec![
          serde_json::json!({"id": "cat1", "name": "Electronics"}),
          serde_json::json!({"id": "cat2", "name": "Books"}),
        ],
      );

    let loader = RelationLoader::new(storage);

    let category_relation = RelationDef::many_to_one("category", "categories", "category_id");

    // Load initial relation
    let initial_docs = vec![
      serde_json::json!({"id": "prod1", "name": "Widget", "category_id": "cat1"}),
      serde_json::json!({"id": "prod2", "name": "Gadget", "category_id": "cat1"}),
    ];

    let loaded = loader
      .load_many(initial_docs, &category_relation, false)
      .await?;
    assert_eq!(loaded.len(), 2);

    // Extract loaded relations
    let prod1_cat = loaded[0]
      .get("category")
      .expect("category should be loaded")
      .clone();
    let prod2_cat = loaded[1]
      .get("category")
      .expect("category should be loaded")
      .clone();

    // Simulate save: serialize to temp file
    let saved_path = temp.path().join("loaded_state.json");
    let state = serde_json::json!({
      "prod1_category": prod1_cat,
      "prod2_category": prod2_cat,
      "loaded_count": loaded.len()
    });
    std::fs::write(&saved_path, serde_json::to_string_pretty(&state).unwrap())?;

    // Reload: read from temp file and verify equality
    let reloaded_raw = std::fs::read_to_string(&saved_path)?;
    let reloaded_state: serde_json::Value = serde_json::from_str(&reloaded_raw).unwrap();

    assert_eq!(reloaded_state["loaded_count"], 2);
    assert_eq!(
      reloaded_state["prod1_category"]["id"],
      serde_json::json!("cat1")
    );
    assert_eq!(
      reloaded_state["prod1_category"]["name"],
      serde_json::json!("Electronics")
    );
    assert_eq!(
      reloaded_state["prod2_category"]["id"],
      serde_json::json!("cat1")
    );
    assert_eq!(
      reloaded_state["prod2_category"]["name"],
      serde_json::json!("Electronics")
    );

    // Assert equality of reloaded category
    assert_eq!(prod1_cat, reloaded_state["prod1_category"]);
    assert_eq!(prod2_cat, reloaded_state["prod2_category"]);

    Ok(())
  }
}
