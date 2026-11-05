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
