use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use crate::error::{OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  DatabaseProvider, FieldInfo, IndexInfo, RawResult, SchemaIntrospection, TransactionControl,
  TransactionId,
};
use crate::query::Filter;
use crate::utils::{compare_values, generate_id, get_document_id_string};

type Store = Arc<RwLock<HashMap<String, Vec<Value>>>>;

/// JSON file-backed provider.
///
/// Each collection is stored as a JSON array in `<base_dir>/<collection>.json`.
/// All reads/writes go through an in-memory cache protected by an async `RwLock`,
/// then flushed to disk.
#[derive(Clone)]
pub struct JsonProvider {
  base_dir: PathBuf,
  cache: Store,
  transaction_manager: Arc<Mutex<Option<TransactionId>>>,
}

impl JsonProvider {
  /// Create (or open) a JSON database at `base_dir`.
  pub async fn new(base_dir: impl AsRef<Path>) -> OrmResult<Self> {
    let base_dir = base_dir.as_ref().to_path_buf();
    tokio::fs::create_dir_all(&base_dir).await?;

    Ok(Self {
      base_dir,
      cache: Arc::new(RwLock::new(HashMap::new())),
      transaction_manager: Arc::new(Mutex::new(None)),
    })
  }

  // ── Private helpers ────────────────────────────────────────────────────

  pub(crate) fn collection_path(&self, collection: &str) -> PathBuf {
    self.base_dir.join(format!("{}.json", collection))
  }

  /// Load a collection from disk into the cache (if not already cached).
  pub(crate) async fn ensure_loaded(&self, collection: &str) -> OrmResult<()> {
    {
      let r = self.cache.read().await;
      if r.contains_key(collection) {
        return Ok(());
      }
    }

    let path = self.collection_path(collection);
    let records: Vec<Value> = if path.exists() {
      let raw = tokio::fs::read_to_string(&path).await?;
      serde_json::from_str(&raw)?
    } else {
      vec![]
    };

    let mut w = self.cache.write().await;
    w.entry(collection.to_string()).or_insert(records);
    Ok(())
  }

  /// Persist the in-memory collection to its JSON file.
  pub(crate) async fn flush(&self, collection: &str) -> OrmResult<()> {
    let r = self.cache.read().await;
    if let Some(records) = r.get(collection) {
      let path = self.collection_path(collection);
      let json_str = serde_json::to_string_pretty(records)?;
      tokio::fs::write(&path, json_str).await?;
    }
    Ok(())
  }

  fn id_of(doc: &Value) -> Option<&str> {
    crate::utils::get_document_id(doc)
  }
}

#[async_trait]
impl DatabaseProvider for JsonProvider {
  async fn insert(&self, collection: &str, mut doc: Value) -> OrmResult<Value> {
    self.ensure_loaded(collection).await?;

    if doc
      .get("id")
      .and_then(|v| v.as_str())
      .is_none_or(|s| s.is_empty())
    {
      doc["id"] = json!(generate_id());
    }

    let mut w = self.cache.write().await;
    let records = w.entry(collection.to_string()).or_default();

    let id = get_document_id_string(&doc)?;
    if records.iter().any(|r| Self::id_of(r) == Some(&id)) {
      return Err(OrmError::Duplicate(format!("id={}", id)));
    }
    records.push(doc.clone());
    drop(w);

    self.flush(collection).await?;
    Ok(doc)
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    self.ensure_loaded(collection).await?;
    let r = self.cache.read().await;
    Ok(
      r.get(collection)
        .and_then(|recs| recs.iter().find(|d| Self::id_of(d) == Some(id)))
        .cloned(),
    )
  }

  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<&str>,
    sort_asc: bool,
  ) -> OrmResult<Vec<Value>> {
    self.ensure_loaded(collection).await?;
    let r = self.cache.read().await;
    let records = match r.get(collection) {
      Some(v) => v,
      None => return Ok(vec![]),
    };

    let mut results: Vec<Value> = records
      .iter()
      .filter(|d| filter.is_none_or(|f| f.matches(d)))
      .cloned()
      .collect();

    // Sort
    if let Some(field) = sort_by {
      results.sort_by(|a, b| {
        let av = a.get(field);
        let bv = b.get(field);
        let ord = compare_values(av, bv);
        if sort_asc {
          ord
        } else {
          ord.reverse()
        }
      });
    }

    // Pagination
    let skip = skip.unwrap_or(0) as usize;
    let results: Vec<Value> = results.into_iter().skip(skip).collect();
    let results = match limit {
      Some(n) => results.into_iter().take(n as usize).collect(),
      None => results,
    };

    Ok(results)
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    self.ensure_loaded(collection).await?;
    let mut w = self.cache.write().await;
    let records = w
      .get_mut(collection)
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;

    let pos = records
      .iter()
      .position(|r| Self::id_of(r) == Some(id))
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;

    records[pos] = doc.clone();
    drop(w);
    self.flush(collection).await?;
    Ok(doc)
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    self.ensure_loaded(collection).await?;
    let mut w = self.cache.write().await;
    let records = w
      .get_mut(collection)
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;

    let pos = records
      .iter()
      .position(|r| Self::id_of(r) == Some(id))
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;

    if let (Value::Object(base), Value::Object(updates)) = (&mut records[pos], patch) {
      for (k, v) in updates {
        base.insert(k, v);
      }
    }
    let updated = records[pos].clone();
    drop(w);
    self.flush(collection).await?;
    Ok(updated)
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    self.ensure_loaded(collection).await?;
    let mut w = self.cache.write().await;
    let records = match w.get_mut(collection) {
      Some(r) => r,
      None => return Ok(false),
    };

    let before = records.len();
    records.retain(|r| Self::id_of(r) != Some(id));
    let removed = records.len() < before;
    drop(w);

    if removed {
      self.flush(collection).await?;
    }
    Ok(removed)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    self.ensure_loaded(collection).await?;
    let r = self.cache.read().await;
    let count = r
      .get(collection)
      .map(|recs| {
        recs
          .iter()
          .filter(|d| filter.is_none_or(|f| f.matches(d)))
          .count()
      })
      .unwrap_or(0);
    Ok(count as u64)
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    self.ensure_loaded(collection).await?;
    let mut w = self.cache.write().await;
    let records = w
      .get_mut(collection)
      .ok_or_else(|| OrmError::NotFound(format!("collection={}", collection)))?;

    let mut count = 0;
    for record in records.iter_mut() {
      if filter.as_ref().is_none_or(|f| f.matches(record)) {
        if let (Value::Object(base), Value::Object(patch)) = (record, &updates) {
          for (k, v) in patch {
            base.insert(k.clone(), v.clone());
          }
        }
        count += 1;
      }
    }
    drop(w);

    if count > 0 {
      self.flush(collection).await?;
    }
    Ok(count)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    self.ensure_loaded(collection).await?;
    let mut w = self.cache.write().await;
    let records = match w.get_mut(collection) {
      Some(r) => r,
      None => return Ok(0),
    };

    let before = records.len();
    records.retain(|r| filter.as_ref().is_some_and(|f| !f.matches(r)));
    let deleted = before - records.len();
    drop(w);

    if deleted > 0 {
      self.flush(collection).await?;
    }
    Ok(deleted)
  }

  // ── Index Management (No-op for JSON provider) ──────────────────────────────────

  /// JSON provider does not support indexes natively.
  /// This is a no-op that logs a warning.
  async fn create_index(&self, _collection: &str, _index: &NosqlIndex) -> OrmResult<()> {
    log::warn!("Indexes are not supported by the JSON provider");
    Ok(())
  }

  /// JSON provider does not support indexes natively.
  /// This is a no-op that logs a warning.
  async fn drop_index(&self, _collection: &str, _index_name: &str) -> OrmResult<()> {
    log::warn!("Indexes are not supported by the JSON provider");
    Ok(())
  }

  /// JSON provider does not support indexes natively.
  /// Returns empty list.
  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Ok(vec![])
  }

  async fn health_check(&self) -> OrmResult<bool> {
    Ok(true)
  }

  async fn insert_many(&self, collection: &str, docs: Vec<Value>) -> OrmResult<usize> {
    let mut count = 0;
    for doc in docs {
      self.insert(collection, doc).await?;
      count += 1;
    }
    Ok(count)
  }

  async fn aggregate(&self, collection: &str, pipeline: Vec<Value>) -> OrmResult<Vec<Value>> {
    use crate::aggregation::AggregationPipeline;
    let all_docs = self.find_all(collection).await?;
    let pipeline = AggregationPipeline::from(pipeline);
    pipeline.execute_docs(all_docs).await
  }
}

#[async_trait]
impl SchemaIntrospection for JsonProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let cache = self.cache.read().await;
    let mut collections = Vec::new();
    for (name, docs) in cache.iter() {
      let size = docs
        .iter()
        .map(|d| serde_json::to_string(d).map(|s| s.len()).unwrap_or(0))
        .sum::<usize>() as u64;
      collections.push(CollectionMeta {
        name: name.clone(),
        document_count: docs.len() as u64,
        size_bytes: size,
        created_at: None,
        updated_at: None,
      });
    }
    Ok(collections)
  }

  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema> {
    self.ensure_loaded(collection).await?;
    let cache = self.cache.read().await;
    if let Some(docs) = cache.get(collection) {
      if let Some(Value::Object(obj)) = docs.first() {
        let mut fields = HashMap::new();
        for (k, v) in obj {
          let field_type = match v {
            Value::Null => "null".to_string(),
            Value::Bool(_) => "boolean".to_string(),
            Value::Number(_) => "number".to_string(),
            Value::String(_) => "string".to_string(),
            Value::Array(_) => "array".to_string(),
            Value::Object(_) => "object".to_string(),
          };
          fields.insert(
            k.clone(),
            FieldInfo {
              name: k.clone(),
              field_type,
              nullable: true,
              default_value: None,
            },
          );
        }
        return Ok(CollectionSchema {
          name: collection.to_string(),
          fields,
          indexes: vec![],
          options: Default::default(),
        });
      }
    }
    Ok(CollectionSchema {
      name: collection.to_string(),
      fields: HashMap::new(),
      indexes: vec![],
      options: Default::default(),
    })
  }

  async fn get_collection_stats(&self, collection: &str) -> OrmResult<CollectionStats> {
    self.ensure_loaded(collection).await?;
    let cache = self.cache.read().await;
    let (document_count, size_bytes) = if let Some(docs) = cache.get(collection) {
      let size = docs
        .iter()
        .map(|d| serde_json::to_string(d).map(|s| s.len()).unwrap_or(0))
        .sum::<usize>() as u64;
      (docs.len() as u64, size)
    } else {
      (0, 0)
    };
    Ok(CollectionStats {
      name: collection.to_string(),
      document_count,
      size_bytes,
      storage_size_bytes: size_bytes,
      index_count: 0,
      index_size_bytes: 0,
      average_document_size: if document_count > 0 {
        size_bytes / document_count
      } else {
        0
      },
    })
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Ok(vec![])
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    Ok("json_file".to_string())
  }
}

#[async_trait]
impl AdminCommands for JsonProvider {
  async fn execute_raw(&self, _query: &str, _params: Vec<Value>) -> OrmResult<RawResult> {
    Err(OrmError::NotSupported(
      "Raw execution not supported for JSON provider".to_string(),
    ))
  }

  async fn create_collection(
    &self,
    collection: &str,
    _schema: Option<CollectionSchema>,
  ) -> OrmResult<()> {
    self.ensure_loaded(collection).await?;
    let mut cache = self.cache.write().await;
    cache.entry(collection.to_string()).or_insert(vec![]);
    drop(cache);
    self.flush(collection).await
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    let path = self.collection_path(collection);
    let mut cache = self.cache.write().await;
    cache.remove(collection);
    drop(cache);
    if path.exists() {
      tokio::fs::remove_file(&path).await?;
    }
    Ok(())
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    let mut cache = self.cache.write().await;
    if let Some(docs) = cache.remove(from) {
      cache.insert(to.to_string(), docs);
    } else {
      return Err(OrmError::NotFound(format!(
        "Collection '{}' not found",
        from
      )));
    }
    drop(cache);

    let from_path = self.collection_path(from);
    let to_path = self.collection_path(to);

    if from_path.exists() {
      tokio::fs::rename(&from_path, &to_path).await?;
    }
    Ok(())
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    Ok("N/A".to_string())
  }

  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    Ok(ConnectionHealth {
      healthy: true,
      latency_ms: None,
      server_version: Some("N/A".to_string()),
      connected_at: None,
      pool_stats: None,
    })
  }
}

#[async_trait]
impl TransactionControl for JsonProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    let mut guard = self.transaction_manager.lock().unwrap();
    if guard.is_some() {
      return Err(OrmError::Transaction(
        "Transaction already active".to_string(),
      ));
    }
    let id = TransactionId::new(uuid::Uuid::new_v4().to_string());
    *guard = Some(id.clone());
    Ok(id)
  }

  async fn commit_transaction(&self, id: TransactionId) -> OrmResult<()> {
    let mut guard = self.transaction_manager.lock().unwrap();
    match guard.as_ref() {
      Some(active_id) if active_id == &id => {
        *guard = None;
        Ok(())
      }
      Some(_) => Err(OrmError::Transaction("Transaction ID mismatch".to_string())),
      None => Err(OrmError::Transaction("No active transaction".to_string())),
    }
  }

  async fn rollback_transaction(&self, id: TransactionId) -> OrmResult<()> {
    let mut guard = self.transaction_manager.lock().unwrap();
    match guard.as_ref() {
      Some(active_id) if active_id == &id => {
        *guard = None;
        Ok(())
      }
      Some(_) => Err(OrmError::Transaction("Transaction ID mismatch".to_string())),
      None => Err(OrmError::Transaction("No active transaction".to_string())),
    }
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(self.transaction_manager.lock().unwrap().is_some())
  }
}
