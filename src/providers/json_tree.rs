use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::admin_types::CollectionOptions;
use crate::error::{OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth, FieldInfo,
  IndexInfo, ProviderCapabilities, RawResult, SchemaIntrospection, Storage, TransactionControl,
  TransactionId,
};
use crate::query::Filter;
use crate::utils::{compare_values, get_document_id_string};

type Store = Arc<RwLock<HashMap<String, Vec<Value>>>>;

fn validate_collection_name(name: &str) -> OrmResult<()> {
  if name.is_empty() {
    return Err(OrmError::InvalidInput(
      "Collection name cannot be empty".to_string(),
    ));
  }
  for seg in name.split('/') {
    if seg.is_empty() {
      return Err(OrmError::InvalidInput(
        "Collection name has an empty segment".to_string(),
      ));
    }
    if seg == ".." {
      return Err(OrmError::InvalidInput(
        "Collection name contains '..' segment".to_string(),
      ));
    }
    if seg.starts_with('.') {
      return Err(OrmError::InvalidInput(
        "Collection name segment cannot start with a dot".to_string(),
      ));
    }
    if seg.contains('\\') {
      return Err(OrmError::InvalidInput(
        "Collection name contains invalid character".to_string(),
      ));
    }
  }
  Ok(())
}

fn collection_path(base_dir: &Path, collection: &str) -> OrmResult<PathBuf> {
  validate_collection_name(collection)?;
  Ok(base_dir.join(collection))
}

fn doc_path(base_dir: &Path, collection: &str, id: &str) -> OrmResult<PathBuf> {
  validate_collection_name(collection)?;
  // id may contain '.' which maps to '/' on disk.
  let rel = id.replace('.', "/");
  if rel.contains("..") || rel.starts_with('/') || rel.starts_with('.') {
    return Err(OrmError::InvalidInput(format!(
      "invalid document id: {}",
      id
    )));
  }
  Ok(base_dir.join(collection).join(format!("{}.json", rel)))
}

fn id_of(doc: &Value) -> Option<String> {
  doc
    .get("id")
    .and_then(|v| v.as_str())
    .map(|s| s.to_string())
}

#[derive(Clone)]
pub struct JsonTreeProvider {
  base_dir: PathBuf,
  cache: Store,
}

impl JsonTreeProvider {
  pub fn new(base_dir: impl AsRef<Path>) -> OrmResult<Self> {
    let base_dir = base_dir.as_ref().to_path_buf();
    std::fs::create_dir_all(&base_dir).map_err(OrmError::Io)?;
    Ok(Self {
      base_dir,
      cache: Arc::new(RwLock::new(HashMap::new())),
    })
  }

  async fn ensure_loaded(&self, collection: &str) -> OrmResult<()> {
    validate_collection_name(collection)?;
    {
      let r = self.cache.read().await;
      if r.contains_key(collection) {
        return Ok(());
      }
    }
    let dir = collection_path(&self.base_dir, collection)?;
    let mut docs: Vec<Value> = Vec::new();
    if dir.exists() {
      // Walk recursively for *.json files.
      fn walk(dir: &Path, out: &mut Vec<Value>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
          for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
              walk(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
              if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(value) = serde_json::from_str::<Value>(&content) {
                  out.push(value);
                }
              }
            }
          }
        }
      }
      walk(&dir, &mut docs);
    }
    let mut w = self.cache.write().await;
    w.insert(collection.to_string(), docs);
    Ok(())
  }

  async fn flush_document(&self, collection: &str, doc: &Value) -> OrmResult<()> {
    let id = get_document_id_string(doc)?;
    let path = doc_path(&self.base_dir, collection, &id)?;
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(OrmError::Io)?;
    }
    let content = serde_json::to_string_pretty(doc).map_err(OrmError::Serialization)?;
    // atomic write
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content).map_err(OrmError::Io)?;
    std::fs::rename(&tmp, &path).map_err(OrmError::Io)?;
    Ok(())
  }

  async fn remove_document_file(&self, collection: &str, id: &str) -> OrmResult<()> {
    let path = doc_path(&self.base_dir, collection, id)?;
    if path.exists() {
      let _ = std::fs::remove_file(&path);
    }
    Ok(())
  }
}

#[async_trait]
impl Storage for JsonTreeProvider {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    self.ensure_loaded(collection).await?;
    if doc
      .get("id")
      .and_then(|v| v.as_str())
      .is_none_or(|s| s.is_empty())
    {
      return Err(OrmError::Validation(
        "document must carry a non-empty 'id'".to_string(),
      ));
    }
    // reject duplicate
    {
      let r = self.cache.read().await;
      if let Some(recs) = r.get(collection) {
        let id = get_document_id_string(&doc)?;
        if recs
          .iter()
          .any(|r| id_of(r).as_deref() == Some(id.as_str()))
        {
          return Err(OrmError::Duplicate(format!("id={}", id)));
        }
      }
    }
    self.flush_document(collection, &doc).await?;
    let mut w = self.cache.write().await;
    let records = w.entry(collection.to_string()).or_default();
    records.push(doc.clone());
    Ok(doc)
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    self.ensure_loaded(collection).await?;
    let r = self.cache.read().await;
    let result = r
      .get(collection)
      .and_then(|recs| recs.iter().find(|d| id_of(d).as_deref() == Some(id)))
      .cloned();
    Ok(result)
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
    let skip = skip.unwrap_or(0) as usize;
    let results: Vec<Value> = results.into_iter().skip(skip).collect();
    let results = match limit {
      Some(n) => results.into_iter().take(n as usize).collect(),
      None => results,
    };
    Ok(results)
  }

  async fn update(&self, collection: &str, id: &str, mut doc: Value) -> OrmResult<Value> {
    self.ensure_loaded(collection).await?;
    if let Value::Object(ref mut obj) = doc {
      obj.insert("id".to_string(), serde_json::json!(id));
    }
    // validate existence
    {
      let r = self.cache.read().await;
      let exists = r
        .get(collection)
        .map(|recs| recs.iter().any(|d| id_of(d).as_deref() == Some(id)))
        .unwrap_or(false);
      if !exists {
        return Err(OrmError::NotFound(format!("{}/{}", collection, id)));
      }
    }
    self.flush_document(collection, &doc).await?;
    let mut w = self.cache.write().await;
    let records = w
      .get_mut(collection)
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;
    let pos = records
      .iter()
      .position(|r| id_of(r).as_deref() == Some(id))
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;
    records[pos] = doc.clone();
    Ok(doc)
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    self.ensure_loaded(collection).await?;
    // read existing
    let existing = self
      .find_by_id(collection, id)
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;
    let mut merged = existing.clone();
    if let (Value::Object(base), Value::Object(updates)) = (&mut merged, patch) {
      for (k, v) in updates {
        if k != "$inc" {
          base.insert(k, v);
        }
      }
      // handle $inc
      // (simplified: handled by caller via update in most cases)
    }
    self.update(collection, id, merged).await
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    self.ensure_loaded(collection).await?;
    let removed = {
      let mut w = self.cache.write().await;
      let records = match w.get_mut(collection) {
        Some(r) => r,
        None => return Ok(false),
      };
      let before = records.len();
      records.retain(|r| id_of(r).as_deref() != Some(id));
      records.len() < before
    };
    if removed {
      self.remove_document_file(collection, id).await?;
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
    // Collect ids to update
    let ids: Vec<String> = {
      let r = self.cache.read().await;
      r.get(collection)
        .map(|recs| {
          recs
            .iter()
            .filter(|d| filter.as_ref().is_none_or(|f| f.matches(d)))
            .filter_map(id_of)
            .collect()
        })
        .unwrap_or_default()
    };
    let mut count = 0;
    for id in ids {
      let existing = self
        .find_by_id(collection, &id)
        .await?
        .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;
      let mut merged = existing;
      if let (Value::Object(base), Value::Object(patch)) = (&mut merged, &updates) {
        for (k, v) in patch {
          if k != "$inc" {
            base.insert(k.clone(), v.clone());
          }
        }
      }
      self.update(collection, &id, merged).await?;
      count += 1;
    }
    Ok(count)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    self.ensure_loaded(collection).await?;
    let ids: Vec<String> = {
      let r = self.cache.read().await;
      r.get(collection)
        .map(|recs| {
          recs
            .iter()
            .filter(|d| filter.as_ref().is_some_and(|f| f.matches(d)))
            .filter_map(id_of)
            .collect()
        })
        .unwrap_or_default()
    };
    let mut deleted = 0;
    for id in ids {
      if self.delete(collection, &id).await? {
        deleted += 1;
      }
    }
    Ok(deleted)
  }

  async fn create_index(&self, _collection: &str, _index: &NosqlIndex) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the JSON tree provider stores documents in files and has no index engine".to_string(),
    ))
  }
  async fn drop_index(&self, _collection: &str, _index_name: &str) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the JSON tree provider stores documents in files and has no index engine".to_string(),
    ))
  }
  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Err(OrmError::NotSupported(
      "the JSON tree provider stores documents in files and has no index engine".to_string(),
    ))
  }

  fn capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities::none()
  }

  async fn health_check(&self) -> OrmResult<bool> {
    Ok(true)
  }

  async fn find_all(&self, collection: &str) -> OrmResult<Vec<Value>> {
    self
      .find_many(collection, None, None, None, None, true)
      .await
  }

  async fn exists(&self, collection: &str, id: &str) -> OrmResult<bool> {
    Ok(self.find_by_id(collection, id).await?.is_some())
  }
}

// SchemaIntrospection
#[async_trait]
impl SchemaIntrospection for JsonTreeProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let mut out = Vec::new();
    if self.base_dir.exists() {
      for entry in std::fs::read_dir(&self.base_dir).map_err(OrmError::Io)? {
        let entry = entry.map_err(OrmError::Io)?;
        if entry.path().is_dir() {
          if let Some(name) = entry.file_name().to_str() {
            out.push(CollectionMeta {
              name: name.to_string(),
              document_count: 0,
              size_bytes: 0,
              created_at: None,
              updated_at: None,
            });
          }
        }
      }
    }
    Ok(out)
  }
  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema> {
    self.ensure_loaded(collection).await?;
    let r = self.cache.read().await;
    let sample = r.get(collection).and_then(|v| v.first()).cloned();
    drop(r);
    match sample {
      Some(doc) => {
        let mut fields = HashMap::new();
        if let Some(obj) = doc.as_object() {
          for (k, v) in obj {
            fields.insert(
              k.clone(),
              FieldInfo {
                name: k.clone(),
                field_type: format!("{}", v),
                nullable: true,
                default_value: None,
              },
            );
          }
        }
        Ok(CollectionSchema {
          name: collection.to_string(),
          fields,
          indexes: vec![],
          options: CollectionOptions::default(),
        })
      }
      None => Ok(CollectionSchema {
        name: collection.to_string(),
        fields: HashMap::new(),
        indexes: vec![],
        options: CollectionOptions::default(),
      }),
    }
  }
  async fn get_collection_stats(&self, collection: &str) -> OrmResult<CollectionStats> {
    self.ensure_loaded(collection).await?;
    let r = self.cache.read().await;
    let count = r.get(collection).map(|v| v.len()).unwrap_or(0);
    Ok(CollectionStats {
      name: collection.to_string(),
      document_count: count as u64,
      size_bytes: 0,
      storage_size_bytes: 0,
      index_count: 0,
      index_size_bytes: 0,
      average_document_size: 0,
    })
  }
  async fn get_database_name(&self) -> OrmResult<String> {
    Ok(self.base_dir.to_string_lossy().to_string())
  }
  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Err(OrmError::NotSupported(
      "the JSON tree provider stores documents in files and has no index engine".to_string(),
    ))
  }
}

// AdminCommands
#[async_trait]
impl AdminCommands for JsonTreeProvider {
  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    validate_collection_name(collection)?;
    let dir = collection_path(&self.base_dir, collection)?;
    if dir.exists() {
      std::fs::remove_dir_all(&dir).map_err(OrmError::Io)?;
    }
    let mut w = self.cache.write().await;
    w.remove(collection);
    Ok(())
  }
  async fn rename_collection(&self, old: &str, new: &str) -> OrmResult<()> {
    validate_collection_name(old)?;
    validate_collection_name(new)?;
    let old_dir = collection_path(&self.base_dir, old)?;
    let new_dir = collection_path(&self.base_dir, new)?;
    if old_dir.exists() {
      if let Some(parent) = new_dir.parent() {
        std::fs::create_dir_all(parent).map_err(OrmError::Io)?;
      }
      std::fs::rename(&old_dir, &new_dir).map_err(OrmError::Io)?;
    }
    let mut w = self.cache.write().await;
    if let Some(v) = w.remove(old) {
      w.insert(new.to_string(), v);
    }
    Ok(())
  }
  async fn execute_raw(&self, _sql: &str, _params: Vec<Value>) -> OrmResult<RawResult> {
    Err(OrmError::NotSupported(
      "execute_raw not supported".to_string(),
    ))
  }
  async fn create_collection(
    &self,
    name: &str,
    _schema: Option<CollectionSchema>,
  ) -> OrmResult<()> {
    validate_collection_name(name)?;
    let dir = collection_path(&self.base_dir, name)?;
    std::fs::create_dir_all(&dir).map_err(OrmError::Io)?;
    Ok(())
  }
  async fn get_server_version(&self) -> OrmResult<String> {
    Ok("json-tree".to_string())
  }
  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    Ok(ConnectionHealth {
      healthy: true,
      latency_ms: None,
      server_version: None,
      connected_at: None,
      pool_stats: None,
    })
  }
}

// The JSON tree provider also persists per call; it has no write buffer to
// commit or roll back.
#[async_trait]
impl TransactionControl for JsonTreeProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    Err(OrmError::NotSupported(
      "the JSON tree provider cannot open a transaction: writes are persisted per call"
        .to_string(),
    ))
  }
  async fn commit_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the JSON tree provider cannot commit a transaction".to_string(),
    ))
  }
  async fn rollback_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the JSON tree provider cannot roll back a transaction".to_string(),
    ))
  }
  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(false)
  }
}
