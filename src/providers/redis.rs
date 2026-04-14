use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use async_trait::async_trait;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde_json::Value;

#[derive(Clone)]
pub struct RedisProvider {
  conn: ConnectionManager,
  prefix: String,
}

impl RedisProvider {
  pub async fn new(connection_string: &str) -> OrmResult<Self> {
    let client =
      redis::Client::open(connection_string).map_err(|e| OrmError::Connection(e.to_string()))?;
    let conn = ConnectionManager::new(client)
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    Ok(Self {
      conn,
      prefix: "nosql_orm:".to_string(),
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
    let _: () = conn.sadd(&self.collection_key(collection), &id).await?;

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
        if let Some(ref f) = filter {
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
      let _: () = conn.srem(&self.collection_key(collection), id).await?;
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
}

impl RedisProvider {
  pub async fn publish(&self, channel: &str, message: &Value) -> OrmResult<()> {
    let mut conn = self.conn.clone();
    let msg = serde_json::to_string(message)?;
    let _: () = conn.publish(channel, msg).await?;
    Ok(())
  }

  pub async fn subscribe(&self, _channel: &str) -> OrmResult<()> {
    Err(OrmError::Connection(
      "subscribe not yet implemented".to_string(),
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
