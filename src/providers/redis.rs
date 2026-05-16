use crate::error::{map_err_connection, OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  DatabaseProvider, IndexInfo, RawResult, SchemaIntrospection, TransactionControl, TransactionId,
};
use crate::query::Filter;
use async_trait::async_trait;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct RedisProvider {
  conn: ConnectionManager,
  prefix: String,
  transaction_manager: Arc<tokio::sync::Mutex<Option<TransactionId>>>,
}

impl RedisProvider {
  pub async fn new(connection_string: &str) -> OrmResult<Self> {
    let client = map_err_connection(redis::Client::open(connection_string))?;
    let conn = map_err_connection(ConnectionManager::new(client).await)?;
    Ok(Self {
      conn,
      prefix: "nosql_orm:".to_string(),
      transaction_manager: Arc::new(tokio::sync::Mutex::new(None)),
    })
  }

  pub fn with_prefix(mut self, prefix: &str) -> Self {
    self.prefix = prefix.to_string();
    self
  }

  fn key(&self, collection: &str, id: &str) -> String {
    format!("{}{}:{}", self.prefix, collection, id)
  }

  fn collection_key(&self, collection: &str) -> String {
    format!("{}{}:ids", self.prefix, collection)
  }
}

#[async_trait]
impl DatabaseProvider for RedisProvider {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut doc_with_id = doc.clone();
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("id".to_string(), Value::String(id.clone()));
    }

    let key = self.key(collection, &id);
    let json = serde_json::to_string(&doc_with_id)?;

    let mut conn = self.conn.clone();
    let _: () = conn.set(&key, &json).await?;
    let _: () = conn.sadd(self.collection_key(collection), &id).await?;

    Ok(doc_with_id)
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let key = self.key(collection, id);
    let mut conn = self.conn.clone();

    let result: Option<String> = conn.get(&key).await?;

    Ok(result.map(|s| serde_json::from_str(&s).unwrap_or(Value::String(s))))
  }

  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    _sort_by: Option<&str>,
    _sort_asc: bool,
  ) -> OrmResult<Vec<Value>> {
    let collection_key = self.collection_key(collection);
    let mut conn = self.conn.clone();

    let ids: Vec<String> = conn.smembers(&collection_key).await?;

    let mut results = Vec::new();
    let skip_usize = skip.unwrap_or(0) as usize;
    let limit_usize = limit.unwrap_or(u64::MAX) as usize;

    for (i, id) in ids.iter().enumerate() {
      if i < skip_usize {
        continue;
      }
      if results.len() >= limit_usize {
        break;
      }

      if let Some(doc) = self.find_by_id(collection, id).await? {
        if let Some(f) = filter {
          if f.matches(&doc) {
            results.push(doc);
          }
        } else {
          results.push(doc);
        }
      }
    }

    Ok(results)
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let key = self.key(collection, id);
    let json = serde_json::to_string(&doc)?;

    let mut conn = self.conn.clone();
    let _: () = conn.set(&key, &json).await?;

    Ok(doc)
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
    let key = self.key(collection, id);
    let mut conn = self.conn.clone();

    let exists: bool = conn.exists(&key).await?;
    if exists {
      let _: () = conn.del(&key).await?;
      let _: () = conn.srem(self.collection_key(collection), id).await?;
      Ok(true)
    } else {
      Ok(false)
    }
  }

  async fn count(&self, collection: &str, _filter: Option<&Filter>) -> OrmResult<u64> {
    let collection_key = self.collection_key(collection);
    let mut conn = self.conn.clone();

    let count: u64 = conn.scard(&collection_key).await?;
    Ok(count)
  }

  async fn exists(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let key = self.key(collection, id);
    let mut conn = self.conn.clone();
    let exists: bool = conn.exists(&key).await?;
    Ok(exists)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let collection_key = self.collection_key(collection);
    let mut conn = self.conn.clone();

    let ids: Vec<String> = conn.smembers(&collection_key).await?;

    let mut to_delete = Vec::new();
    for id in &ids {
      if let Some(doc) = self.find_by_id(collection, id).await? {
        if filter.as_ref().is_none_or(|f| f.matches(&doc)) {
          to_delete.push(id.clone());
        }
      }
    }

    if to_delete.is_empty() {
      return Ok(0);
    }

    let mut keys: Vec<String> = to_delete
      .iter()
      .map(|id| self.key(collection, id))
      .collect();
    keys.push(collection_key.clone());

    let _: () = redis::cmd("DEL").arg(&keys).query_async(&mut conn).await?;

    let count = to_delete.len();
    Ok(count)
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    let collection_key = self.collection_key(collection);
    let mut conn = self.conn.clone();

    let ids: Vec<String> = conn.smembers(&collection_key).await?;

    let mut to_update = Vec::new();
    for id in &ids {
      if let Some(doc) = self.find_by_id(collection, id).await? {
        if filter.as_ref().is_none_or(|f| f.matches(&doc)) {
          to_update.push((id.clone(), doc));
        }
      }
    }

    if to_update.is_empty() {
      return Ok(0);
    }

    let count = to_update.len();
    let mut pipe = redis::pipe();
    for (id, mut doc) in to_update {
      if let (Some(updates_obj), Some(doc_obj)) = (updates.as_object(), doc.as_object_mut()) {
        for (k, v) in updates_obj {
          doc_obj.insert(k.clone(), v.clone());
        }
      }
      let key = self.key(collection, &id);
      let json = serde_json::to_string(&doc)?;
      pipe.set(&key, json);
    }

    pipe.query_async::<_, ()>(&mut conn).await?;

    Ok(count)
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
}

impl RedisProvider {
  pub async fn publish(&self, channel: &str, message: &Value) -> OrmResult<()> {
    let mut conn = self.conn.clone();
    let msg = serde_json::to_string(message)?;
    let _: () = conn.publish(channel, msg).await?;
    Ok(())
  }

  #[doc(hidden)]
  pub async fn subscribe(&self, _channel: &str) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "subscribe not yet implemented for Redis provider".to_string(),
    ))
  }

  pub async fn cache_set(&self, key: &str, value: &Value, ttl_secs: u64) -> OrmResult<()> {
    let mut conn = self.conn.clone();
    let json = serde_json::to_string(value)?;
    let full_key = format!("{}:{}", self.prefix, key);
    let _: () = conn.set_ex(full_key, json, ttl_secs).await?;
    Ok(())
  }

  pub async fn cache_get(&self, key: &str) -> OrmResult<Option<Value>> {
    let mut conn = self.conn.clone();
    let full_key = format!("{}:{}", self.prefix, key);
    let result: Option<String> = conn.get(&full_key).await?;
    Ok(result.map(|s| serde_json::from_str(&s).unwrap_or(Value::String(s))))
  }
}

#[async_trait]
impl SchemaIntrospection for RedisProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let mut conn = self.conn.clone();
    let pattern = format!("{}*:ids", self.prefix);
    let keys: Vec<String> = conn.keys(&pattern).await?;
    let collections = keys
      .iter()
      .map(|k| {
        let name = k
          .strip_prefix(&self.prefix)
          .unwrap_or(k)
          .trim_end_matches(":ids")
          .to_string();
        name
      })
      .collect::<Vec<_>>();
    let mut result = Vec::new();
    for name in collections {
      let count: u64 = conn.scard(format!("{}{}:ids", self.prefix, name)).await?;
      result.push(CollectionMeta {
        name,
        document_count: count,
        size_bytes: 0,
        created_at: None,
        updated_at: None,
      });
    }
    Ok(result)
  }

  async fn describe_collection(&self, _collection: &str) -> OrmResult<CollectionSchema> {
    Ok(CollectionSchema {
      name: _collection.to_string(),
      fields: HashMap::new(),
      indexes: vec![],
      options: Default::default(),
    })
  }

  async fn get_collection_stats(&self, collection: &str) -> OrmResult<CollectionStats> {
    let mut conn = self.conn.clone();
    let count: u64 = conn.scard(self.collection_key(collection)).await?;
    Ok(CollectionStats {
      name: collection.to_string(),
      document_count: count,
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
    Ok("redis".to_string())
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    Ok(vec!["redis".to_string()])
  }
}

#[async_trait]
impl AdminCommands for RedisProvider {
  async fn execute_raw(&self, _query: &str, _params: Vec<Value>) -> OrmResult<RawResult> {
    Err(OrmError::NotSupported(
      "Raw execution not supported for Redis provider".to_string(),
    ))
  }

  async fn create_collection(
    &self,
    collection: &str,
    _schema: Option<CollectionSchema>,
  ) -> OrmResult<()> {
    let mut conn = self.conn.clone();
    let _: () = conn.sadd(self.collection_key(collection), "").await?;
    let _: () = conn.srem(self.collection_key(collection), "").await?;
    Ok(())
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    let mut conn = self.conn.clone();
    let ids: Vec<String> = conn.smembers(self.collection_key(collection)).await?;
    let mut keys: Vec<String> = ids.iter().map(|id| self.key(collection, id)).collect();
    keys.push(self.collection_key(collection));
    if !keys.is_empty() {
      let _: () = redis::cmd("DEL").arg(&keys).query_async(&mut conn).await?;
    }
    Ok(())
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    let mut conn = self.conn.clone();
    let info: String = redis::cmd("INFO")
      .arg("server")
      .query_async(&mut conn)
      .await?;
    let version = info
      .lines()
      .find(|l| l.starts_with("redis_version:"))
      .map(|l| l.split(':').nth(1).unwrap_or("unknown").trim().to_string())
      .unwrap_or_else(|| "unknown".to_string());
    Ok(version)
  }

  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    let healthy = self.health_check().await.unwrap_or(false);
    let server_version = self
      .get_server_version()
      .await
      .unwrap_or_else(|_| "unknown".to_string());
    Ok(ConnectionHealth {
      healthy,
      latency_ms: None,
      server_version: Some(server_version),
      connected_at: None,
      pool_stats: None,
    })
  }

  async fn rename_collection(&self, _from: &str, _to: &str) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "rename_collection not supported for Redis provider".to_string(),
    ))
  }
}

#[async_trait]
impl TransactionControl for RedisProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    let mut guard = self.transaction_manager.lock().await;
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
    let mut guard = self.transaction_manager.lock().await;
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
    let mut guard = self.transaction_manager.lock().await;
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
    Ok(self.transaction_manager.lock().await.is_some())
  }
}
