use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{OrmError, OrmResult};
use crate::nosql_index::{NosqlIndex, NosqlIndexInfo};
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use crate::utils::generate_id;

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
}

impl JsonProvider {
  /// Create (or open) a JSON database at `base_dir`.
  pub async fn new(base_dir: impl AsRef<Path>) -> OrmResult<Self> {
    let base_dir = base_dir.as_ref().to_path_buf();
    tokio::fs::create_dir_all(&base_dir).await?;

    Ok(Self {
      base_dir,
      cache: Arc::new(RwLock::new(HashMap::new())),
    })
  }

  // ── Private helpers ────────────────────────────────────────────────────

  fn collection_path(&self, collection: &str) -> PathBuf {
    self.base_dir.join(format!("{}.json", collection))
  }

  /// Load a collection from disk into the cache (if not already cached).
  async fn ensure_loaded(&self, collection: &str) -> OrmResult<()> {
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
  async fn flush(&self, collection: &str) -> OrmResult<()> {
    let r = self.cache.read().await;
    if let Some(records) = r.get(collection) {
      let path = self.collection_path(collection);
      let json_str = serde_json::to_string_pretty(records)?;
      tokio::fs::write(&path, json_str).await?;
    }
    Ok(())
  }

  fn id_of(doc: &Value) -> Option<&str> {
    doc.get("id").and_then(|v| v.as_str())
  }
}

#[async_trait]
impl DatabaseProvider for JsonProvider {
  async fn insert(&self, collection: &str, mut doc: Value) -> OrmResult<Value> {
    self.ensure_loaded(collection).await?;

    if doc
      .get("id")
      .and_then(|v| v.as_str())
      .map_or(true, |s| s.is_empty())
    {
      doc["id"] = json!(generate_id());
    }

    let mut w = self.cache.write().await;
    let records = w.entry(collection.to_string()).or_default();

    let id = doc["id"].as_str().unwrap().to_string();
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
      Some(v) => v.clone(),
      None => return Ok(vec![]),
    };
    drop(r);

    // Filter
    let mut results: Vec<Value> = records
      .into_iter()
      .filter(|d| filter.map_or(true, |f| f.matches(d)))
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
          .filter(|d| filter.map_or(true, |f| f.matches(d)))
          .count()
      })
      .unwrap_or(0);
    Ok(count as u64)
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
  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<NosqlIndexInfo>> {
    Ok(vec![])
  }
}

fn compare_values(a: Option<&Value>, b: Option<&Value>) -> std::cmp::Ordering {
  use std::cmp::Ordering;
  match (a, b) {
    (Some(Value::Number(n1)), Some(Value::Number(n2))) => n1
      .as_f64()
      .unwrap_or(0.0)
      .partial_cmp(&n2.as_f64().unwrap_or(0.0))
      .unwrap_or(Ordering::Equal),
    (Some(Value::String(s1)), Some(Value::String(s2))) => s1.cmp(s2),
    (Some(_), None) => Ordering::Greater,
    (None, Some(_)) => Ordering::Less,
    _ => Ordering::Equal,
  }
}
