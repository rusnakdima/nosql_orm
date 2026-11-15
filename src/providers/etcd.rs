use crate::providers::kv_common;
// etcd provider — a native key-value backend (feature `etcd`).
//
// etcd stores bytes under hierarchical keys, so a collection is a key prefix
// and a document is one key: `{prefix}{collection}/{id}`. Because `/` is
// rejected in collection names, the prefix is unambiguous and a single range
// read returns a whole collection — no id-set bookkeeping like Redis needs.
//
// etcd's `Txn` gives compare-and-swap, so `insert` is genuinely
// insert-if-absent and `update` genuinely requires an existing key. What it
// does not have is a session spanning several requests, so `TransactionControl`
// answers `NotSupported` rather than handing out an id that cannot undo
// anything.

use crate::error::{map_err_connection, OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth, FieldInfo,
  IndexInfo, ProviderCapabilities, RawResult, SchemaIntrospection, Storage, TransactionControl,
  TransactionId,
};
use crate::query::Filter;
// (compare_values, generate_id now in kv_common)
use async_trait::async_trait;
use etcd_client::{Client, Compare, CompareOp, DeleteOptions, GetOptions, PutOptions, Txn, TxnOp};
use serde_json::Value;
use std::collections::HashMap;



// Collection names become part of a key prefix, so a name containing `/` would
// make the prefix ambiguous. Everything else is left alone.
fn validate_collection(collection: &str) -> OrmResult<()> {
  if collection.is_empty() {
    return Err(OrmError::InvalidInput(
      "Collection name cannot be empty".to_string(),
    ));
  }
  if collection.contains('/') {
    return Err(OrmError::InvalidInput(
      "Collection name cannot contain '/' (it separates the id in an etcd key)".to_string(),
    ));
  }
  Ok(())
}


#[derive(Clone)]
pub struct EtcdProvider {
  client: Client,
  prefix: String,
}

impl EtcdProvider {
  // Connect to one or more etcd endpoints, comma-separated:
  //   http://127.0.0.1:2379
  //   http://node1:2379,http://node2:2379
  pub async fn connect(endpoints: impl AsRef<str>) -> OrmResult<Self> {
    let endpoints: Vec<&str> = endpoints
      .as_ref()
      .split(',')
      .map(str::trim)
      .filter(|s| !s.is_empty())
      .collect();

    if endpoints.is_empty() {
      return Err(OrmError::Connection("no etcd endpoint given".to_string()));
    }

    let client = map_err_connection(Client::connect(endpoints, None).await)?;

    Ok(Self {
      client,
      prefix: "nosql_orm/".to_string(),
    })
  }

  pub async fn from_config(config: &crate::provider::ProviderConfig) -> OrmResult<Self> {
    let provider = Self::connect(&config.connection).await?;
    Ok(match &config.database {
      Some(db) => provider.with_prefix(db),
      None => provider,
    })
  }

  // Key prefix shared by every collection this provider touches.
  pub fn with_prefix(mut self, prefix: &str) -> Self {
    self.prefix = prefix.to_string();
    self
  }

  fn key(&self, collection: &str, id: &str) -> String {
    format!("{}{}/{}", self.prefix, collection, id)
  }

  fn collection_prefix(&self, collection: &str) -> String {
    format!("{}{}/", self.prefix, collection)
  }

  // Read every key under `prefix` as `(key, value)` pairs.
  async fn scan(&self, prefix: &str) -> OrmResult<Vec<(String, Vec<u8>)>> {
    let mut client = self.client.clone();
    let response = map_err_connection(
      client
        .get(prefix.to_string(), Some(GetOptions::new().with_prefix()))
        .await,
    )?;

    let mut pairs = Vec::new();
    for kv in response.kvs() {
      let key = match kv.key_str() {
        Ok(k) => k.to_string(),
        Err(_) => continue,
      };
      pairs.push((key, kv.value().to_vec()));
    }
    Ok(pairs)
  }

  // Read a whole collection, parsing each value as a JSON document.
  async fn scan_collection(&self, collection: &str) -> OrmResult<Vec<Value>> {
    let prefix = self.collection_prefix(collection);
    let pairs = self.scan(&prefix).await?;

    let mut documents = Vec::with_capacity(pairs.len());
    for (_, value) in pairs {
      match serde_json::from_slice::<Value>(&value) {
        Ok(doc) => documents.push(doc),
        // A value that is not JSON is skipped rather than failing the whole
        // read: one corrupt key must not hide every healthy document.
        Err(_) => continue,
      }
    }
    Ok(documents)
  }
}

#[async_trait]
impl Storage for EtcdProvider {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    validate_collection(collection)?;

    let id = kv_common::document_id_or_new(&doc);
    let mut doc_with_id = doc;
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("id".to_string(), Value::String(id.clone()));
    }

    let key = self.key(collection, &id);
    let json = serde_json::to_string(&doc_with_id)?;

    // create_revision == 0 means the key does not exist yet, so the compare
    // makes this a real insert-if-absent instead of a blind overwrite.
    let txn = Txn::new()
      .when(vec![Compare::create_revision(
        key.clone(),
        CompareOp::Equal,
        0,
      )])
      .and_then(vec![TxnOp::put(key.clone(), json, None)]);

    let mut client = self.client.clone();
    let response = map_err_connection(client.txn(txn).await)?;

    if !response.succeeded() {
      return Err(OrmError::Duplicate(format!(
        "{}/{} already exists",
        collection, id
      )));
    }

    Ok(doc_with_id)
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    validate_collection(collection)?;
    let key = self.key(collection, id);

    let mut client = self.client.clone();
    let response = map_err_connection(client.get(key, None).await)?;

    let Some(kv) = response.kvs().first() else {
      return Ok(None);
    };

    match serde_json::from_slice::<Value>(kv.value()) {
      Ok(doc) => Ok(Some(doc)),
      Err(_) => Ok(None),
    }
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
    validate_collection(collection)?;
    let documents = self.scan_collection(collection).await?;
    Ok(kv_common::select_documents(
      documents, filter, skip, limit, sort_by, sort_asc,
    ))
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    validate_collection(collection)?;

    let key = self.key(collection, id);
    let mut doc_with_id = doc;
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("id".to_string(), Value::String(id.to_string()));
    }
    let json = serde_json::to_string(&doc_with_id)?;

    // create_revision != 0 means the key exists: replacing a missing document
    // is an error, not a silent insert.
    let txn = Txn::new()
      .when(vec![Compare::create_revision(
        key.clone(),
        CompareOp::NotEqual,
        0,
      )])
      .and_then(vec![TxnOp::put(key.clone(), json, None)]);

    let mut client = self.client.clone();
    let response = map_err_connection(client.txn(txn).await)?;

    if !response.succeeded() {
      return Err(OrmError::NotFound(format!("{}/{}", collection, id)));
    }

    Ok(doc_with_id)
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let mut doc = self
      .find_by_id(collection, id)
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;

    if let (Some(patch_obj), Some(doc_obj)) = (patch.as_object(), doc.as_object_mut()) {
      for (key, value) in patch_obj {
        doc_obj.insert(key.clone(), value.clone());
      }
    }

    if let Some(obj) = doc.as_object_mut() {
      self
        .update(collection, id, Value::Object(obj.clone()))
        .await
    } else {
      Err(OrmError::InvalidQuery(
        "Document is not an object".to_string(),
      ))
    }
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    validate_collection(collection)?;
    let key = self.key(collection, id);

    // The compare reports whether the key existed, so the caller learns the
    // truth from one round trip rather than a get-then-delete race.
    let txn = Txn::new()
      .when(vec![Compare::create_revision(
        key.clone(),
        CompareOp::NotEqual,
        0,
      )])
      .and_then(vec![TxnOp::delete(key, None)]);

    let mut client = self.client.clone();
    let response = map_err_connection(client.txn(txn).await)?;
    Ok(response.succeeded())
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    validate_collection(collection)?;
    let documents = self.scan_collection(collection).await?;

    let mut ids = Vec::new();
    for doc in &documents {
      if filter.as_ref().is_none_or(|f| f.matches(doc)) {
        if let Some(id) = kv_common::document_id(doc) {
          ids.push(id);
        }
      }
    }

    if ids.is_empty() {
      return Ok(0);
    }

    let mut client = self.client.clone();
    let mut deleted = 0usize;
    for id in &ids {
      let key = self.key(collection, id);
      let txn = Txn::new()
        .when(vec![Compare::create_revision(
          key.clone(),
          CompareOp::NotEqual,
          0,
        )])
        .and_then(vec![TxnOp::delete(key, None)]);
      let response = map_err_connection(client.txn(txn).await)?;
      if response.succeeded() {
        deleted += 1;
      }
    }

    Ok(deleted)
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    validate_collection(collection)?;
    let documents = self.scan_collection(collection).await?;

    let mut updated = 0usize;
    let mut client = self.client.clone();

    for mut doc in documents {
      if !filter.as_ref().is_none_or(|f| f.matches(&doc)) {
        continue;
      }
      let Some(id) = kv_common::document_id(&doc) else {
        continue;
      };

      if let (Some(updates_obj), Some(doc_obj)) = (updates.as_object(), doc.as_object_mut()) {
        for (key, value) in updates_obj {
          doc_obj.insert(key.clone(), value.clone());
        }
      }

      let key = self.key(collection, &id);
      let json = serde_json::to_string(&doc)?;
      let txn = Txn::new()
        .when(vec![Compare::create_revision(
          key.clone(),
          CompareOp::NotEqual,
          0,
        )])
        .and_then(vec![TxnOp::put(key, json, None)]);

      let response = map_err_connection(client.txn(txn).await)?;
      if response.succeeded() {
        updated += 1;
      }
    }

    Ok(updated)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    validate_collection(collection)?;
    let documents = self.scan_collection(collection).await?;

    let Some(filter) = filter else {
      return Ok(documents.len() as u64);
    };

    Ok(documents.iter().filter(|doc| filter.matches(doc)).count() as u64)
  }

  async fn create_index(&self, _collection: &str, _index: &NosqlIndex) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the etcd provider stores plain keys and has no index engine".to_string(),
    ))
  }

  async fn drop_index(&self, _collection: &str, _index_name: &str) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the etcd provider stores plain keys and has no index engine".to_string(),
    ))
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Err(OrmError::NotSupported(
      "the etcd provider stores plain keys and has no index engine".to_string(),
    ))
  }

  fn capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities::none()
  }

  async fn health_check(&self) -> OrmResult<bool> {
    let mut client = self.client.clone();
    Ok(map_err_connection(client.status().await).is_ok())
  }
}

#[async_trait]
impl SchemaIntrospection for EtcdProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    // One range read over the whole prefix: every key is
    // `{prefix}{collection}/{id}`, so the segment before the first `/` names
    // the collection and the count falls out of the grouping.
    let pairs = self.scan(&self.prefix).await?;

    let mut counts: HashMap<String, (u64, u64)> = HashMap::new();
    for (key, value) in &pairs {
      let Some(rest) = key.strip_prefix(&self.prefix) else {
        continue;
      };
      let Some((collection, _)) = rest.split_once('/') else {
        continue;
      };
      if collection.is_empty() {
        continue;
      }
      let entry = counts.entry(collection.to_string()).or_insert((0, 0));
      entry.0 += 1;
      entry.1 += value.len() as u64;
    }

    let mut result: Vec<CollectionMeta> = counts
      .into_iter()
      .map(|(name, (document_count, size_bytes))| CollectionMeta {
        name,
        document_count,
        size_bytes,
        created_at: None,
        updated_at: None,
      })
      .collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
  }

  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema> {
    validate_collection(collection)?;

    // Field names come from the documents themselves; when documents disagree
    // about a field's type the report says `mixed` rather than picking one.
    let documents = self.scan_collection(collection).await?;
    let mut fields: HashMap<String, FieldInfo> = HashMap::new();

    for doc in &documents {
      let Some(obj) = doc.as_object() else {
        continue;
      };
      for (name, value) in obj {
        let kind = value_kind(value);
        match fields.get_mut(name) {
          Some(info) if info.field_type != kind && info.field_type != "mixed" => {
            info.field_type = "mixed".to_string();
          }
          Some(_) => {}
          None => {
            fields.insert(
              name.clone(),
              FieldInfo {
                name: name.clone(),
                field_type: kind.to_string(),
                nullable: true,
                default_value: None,
              },
            );
          }
        }
      }
    }

    let mut fields: Vec<_> = fields.into_iter().collect();
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(CollectionSchema {
      name: collection.to_string(),
      fields: fields.into_iter().collect(),
      indexes: vec![],
      options: Default::default(),
    })
  }

  async fn get_collection_stats(&self, collection: &str) -> OrmResult<CollectionStats> {
    validate_collection(collection)?;
    let prefix = self.collection_prefix(collection);
    let pairs = self.scan(&prefix).await?;

    let document_count = pairs.len() as u64;
    let size_bytes: u64 = pairs.iter().map(|(_, v)| v.len() as u64).sum();

    Ok(CollectionStats {
      name: collection.to_string(),
      document_count,
      size_bytes,
      storage_size_bytes: size_bytes,
      index_count: 0,
      index_size_bytes: 0,
      average_document_size: if document_count == 0 {
        0
      } else {
        size_bytes / document_count
      },
    })
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Err(OrmError::NotSupported(
      "the etcd provider stores plain keys and has no index engine".to_string(),
    ))
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    Ok("etcd".to_string())
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    Ok(vec!["etcd".to_string()])
  }
}

#[async_trait]
impl AdminCommands for EtcdProvider {
  async fn execute_raw(&self, _query: &str, _params: Vec<Value>) -> OrmResult<RawResult> {
    Err(OrmError::NotSupported(
      "etcd has no query language; use the Storage API".to_string(),
    ))
  }

  async fn create_collection(
    &self,
    collection: &str,
    _schema: Option<CollectionSchema>,
  ) -> OrmResult<()> {
    // A collection is an implicit key prefix: it appears when its first
    // document is written and vanishes with its last, so there is nothing to
    // create. Validating the name is the useful part.
    validate_collection(collection)
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    validate_collection(collection)?;
    let prefix = self.collection_prefix(collection);

    let mut client = self.client.clone();
    map_err_connection(
      client
        .delete(prefix, Some(DeleteOptions::new().with_prefix()))
        .await,
    )?;
    Ok(())
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    validate_collection(from)?;
    validate_collection(to)?;

    let from_prefix = self.collection_prefix(from);
    let pairs = self.scan(&from_prefix).await?;

    let mut client = self.client.clone();
    for (key, value) in &pairs {
      let Some(id) = key.strip_prefix(&from_prefix) else {
        continue;
      };
      let new_key = self.key(to, id);
      map_err_connection(
        client
          .put(new_key, value.clone(), Some(PutOptions::default()))
          .await,
      )?;
    }

    map_err_connection(
      client
        .delete(from_prefix, Some(DeleteOptions::new().with_prefix()))
        .await,
    )?;
    Ok(())
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    let mut client = self.client.clone();
    let status = map_err_connection(client.status().await)?;
    Ok(status.version().to_string())
  }

  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    let mut client = self.client.clone();
    let started = std::time::Instant::now();

    match client.status().await {
      Ok(status) => Ok(ConnectionHealth {
        healthy: true,
        latency_ms: Some(started.elapsed().as_millis() as u64),
        server_version: Some(status.version().to_string()),
        connected_at: None,
        pool_stats: None,
      }),
      Err(e) => {
        tracing::warn!("etcd health check failed: {}", e);
        Ok(ConnectionHealth {
          healthy: false,
          latency_ms: None,
          server_version: None,
          connected_at: None,
          pool_stats: None,
        })
      }
    }
  }
}

// etcd's `Txn` is a single atomic request, not a session spanning requests, so
// there is no handle to begin/commit/rollback. Reporting NotSupported keeps the
// promise honest: no id is handed out that cannot undo anything.
#[async_trait]
impl TransactionControl for EtcdProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    Err(OrmError::NotSupported(
      "the etcd provider has no multi-request transaction session".to_string(),
    ))
  }

  async fn commit_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the etcd provider has no multi-request transaction session".to_string(),
    ))
  }

  async fn rollback_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the etcd provider has no multi-request transaction session".to_string(),
    ))
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(false)
  }
}

fn value_kind(value: &Value) -> &'static str {
  match value {
    Value::Null => "null",
    Value::Bool(_) => "boolean",
    Value::Number(_) => "number",
    Value::String(_) => "string",
    Value::Array(_) => "array",
    Value::Object(_) => "object",
  }
}

#[cfg(test)]
mod tests;
