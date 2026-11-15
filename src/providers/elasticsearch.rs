use crate::error::{OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth, FieldInfo,
  IndexInfo, ProviderCapabilities, RawResult, SchemaIntrospection, Storage, TransactionControl,
  TransactionId,
};
use crate::providers::kv_common;
use crate::query::Filter;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;

fn parse_base(input: &str) -> (String, Option<(String, String)>, Option<String>) {
  // returns base, basic creds, api key
  if let Ok(url) = reqwest::Url::parse(input) {
    let user = url.username().to_string();
    let pass = url.password().unwrap_or("").to_string();
    let has = !user.is_empty();
    let mut clean = url.clone();
    let _ = clean.set_username("");
    let _ = clean.set_password(None);
    let base = clean.to_string().trim_end_matches('/').to_string();
    let creds = if has { Some((user, pass)) } else { None };
    (base, creds, None)
  } else {
    (input.trim_end_matches('/').to_string(), None, None)
  }
}

#[derive(Clone)]
pub struct ElasticsearchProvider {
  base: String,
  creds: Option<(String, String)>,
  api_key: Option<String>,
  client: Client,
  options_user: Option<String>,
  options_pass: Option<String>,
  options_api_key: Option<String>,
}

impl ElasticsearchProvider {
  pub async fn connect(base_url: impl AsRef<str>) -> OrmResult<Self> {
    let (base, creds, api_key) = parse_base(base_url.as_ref());
    Ok(Self {
      base,
      creds,
      api_key,
      client: Client::new(),
      options_user: None,
      options_pass: None,
      options_api_key: None,
    })
  }

  pub async fn new(connection: impl AsRef<str>) -> OrmResult<Self> {
    Self::connect(connection).await
  }

  pub async fn from_config(config: &crate::provider::ProviderConfig) -> OrmResult<Self> {
    let (base, creds, api_key) = parse_base(&config.connection);
    let mut p = Self {
      base,
      creds,
      api_key,
      client: Client::new(),
      options_user: None,
      options_pass: None,
      options_api_key: None,
    };
    if let Some(u) = config.options.get("username") {
      p.options_user = Some(u.clone());
    }
    if let Some(pass) = config.options.get("password") {
      p.options_pass = Some(pass.clone());
    }
    if let Some(k) = config.options.get("api_key") {
      p.options_api_key = Some(k.clone());
    }
    Ok(p)
  }

  fn creds(&self) -> Option<(String, String)> {
    if let (Some(u), Some(pass)) = (&self.options_user, &self.options_pass) {
      Some((u.clone(), pass.clone()))
    } else {
      self.creds.clone()
    }
  }

  fn api_key(&self) -> Option<String> {
    self
      .options_api_key
      .clone()
      .or_else(|| self.api_key.clone())
  }

  fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
    let url = format!("{}/{}", self.base, path.trim_start_matches('/'));
    let mut rb = self.client.request(method, url);
    if let Some((u, p)) = self.creds() {
      rb = rb.basic_auth(u, Some(p));
    }
    if let Some(k) = self.api_key() {
      rb = rb.header("Authorization", format!("ApiKey {}", k));
    }
    rb
  }

  async fn read_all(&self, collection: &str) -> OrmResult<Vec<Value>> {
    crate::utils::validate_collection_simple(collection)?;
    let body = serde_json::json!({"query":{"match_all":{}},"size":10000});
    let resp = self
      .request(reqwest::Method::POST, &format!("{}/_search", collection))
      .json(&body)
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if resp.status().as_u16() == 404 {
      return Ok(Vec::new());
    }
    if !resp.status().is_success() {
      let text = resp.text().await.unwrap_or_default();
      return Err(OrmError::Query(format!(
        "Elasticsearch search failed: {}",
        text
      )));
    }
    let body: Value = resp
      .json()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    let mut docs = Vec::new();
    if let Some(hits) = body
      .get("hits")
      .and_then(|h| h.get("hits"))
      .and_then(|v| v.as_array())
    {
      for hit in hits {
        if let Some(src) = hit.get("_source") {
          let mut doc = src.clone();
          if doc.get("id").is_none() {
            if let Some(id) = hit.get("_id").and_then(|v| v.as_str()) {
              if let Some(obj) = doc.as_object_mut() {
                obj.insert("id".to_string(), Value::String(id.to_string()));
              }
            }
          }
          docs.push(doc);
        }
      }
    }
    Ok(docs)
  }
}

#[async_trait]
impl Storage for ElasticsearchProvider {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    crate::utils::validate_collection_simple(collection)?;
    let id = kv_common::document_id_or_new(&doc);
    let mut doc_with_id = doc;
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("id".to_string(), Value::String(id.clone()));
    }
    if self.find_by_id(collection, &id).await?.is_some() {
      return Err(OrmError::Duplicate(format!(
        "{}/{} already exists",
        collection, id
      )));
    }
    let resp = self
      .request(reqwest::Method::PUT, &format!("{}/_doc/{}", collection, id))
      .query(&[("op_type", "create")])
      .json(&doc_with_id)
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
      let text = resp.text().await.unwrap_or_default();
      if status.as_u16() == 409 {
        return Err(OrmError::Duplicate(format!(
          "{}/{} already exists",
          collection, id
        )));
      }
      return Err(OrmError::Query(format!(
        "Elasticsearch insert failed: {}",
        text
      )));
    }
    Ok(doc_with_id)
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    crate::utils::validate_collection_simple(collection)?;
    let resp = self
      .request(reqwest::Method::GET, &format!("{}/_doc/{}", collection, id))
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if resp.status().as_u16() == 404 {
      return Ok(None);
    }
    if !resp.status().is_success() {
      let text = resp.text().await.unwrap_or_default();
      return Err(OrmError::Query(format!(
        "Elasticsearch get failed: {}",
        text
      )));
    }
    let body: Value = resp
      .json()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if let Some(src) = body.get("_source") {
      let mut doc = src.clone();
      if doc.get("id").is_none() {
        if let Some(obj) = doc.as_object_mut() {
          obj.insert("id".to_string(), Value::String(id.to_string()));
        }
      }
      return Ok(Some(doc));
    }
    Ok(None)
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
    let documents = self.read_all(collection).await.unwrap_or_default();
    Ok(kv_common::select_documents(
      documents, filter, skip, limit, sort_by, sort_asc,
    ))
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    crate::utils::validate_collection_simple(collection)?;
    if self.find_by_id(collection, id).await?.is_none() {
      return Err(OrmError::NotFound(format!("{}/{}", collection, id)));
    }
    let mut doc_with_id = doc;
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("id".to_string(), Value::String(id.to_string()));
    }
    let resp = self
      .request(reqwest::Method::PUT, &format!("{}/_doc/{}", collection, id))
      .json(&doc_with_id)
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if !resp.status().is_success() {
      let text = resp.text().await.unwrap_or_default();
      return Err(OrmError::Query(format!(
        "Elasticsearch update failed: {}",
        text
      )));
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
      let id = obj
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or(id)
        .to_string();
      self
        .update(collection, &id, Value::Object(obj.clone()))
        .await
    } else {
      Err(OrmError::InvalidQuery(
        "Document is not an object".to_string(),
      ))
    }
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    crate::utils::validate_collection_simple(collection)?;
    let resp = self
      .request(
        reqwest::Method::DELETE,
        &format!("{}/_doc/{}", collection, id),
      )
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if resp.status().as_u16() == 404 {
      return Ok(false);
    }
    Ok(resp.status().is_success())
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let documents = self.read_all(collection).await?;
    let mut deleted = 0usize;
    for doc in &documents {
      let id = kv_common::document_id(doc).unwrap_or_default();
      if id.is_empty() {
        continue;
      }
      if !filter.as_ref().is_none_or(|f| f.matches(doc)) {
        continue;
      }
      if self.delete(collection, &id).await? {
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
    let documents = self.read_all(collection).await?;
    let mut updated = 0usize;
    for mut doc in documents {
      if !filter.as_ref().is_none_or(|f| f.matches(&doc)) {
        continue;
      }
      let id = kv_common::document_id(&doc).unwrap_or_default();
      if id.is_empty() {
        continue;
      }
      if let (Some(updates_obj), Some(doc_obj)) = (updates.as_object(), doc.as_object_mut()) {
        for (key, value) in updates_obj {
          doc_obj.insert(key.clone(), value.clone());
        }
      }
      if let Some(obj) = doc.as_object_mut() {
        obj.insert("id".to_string(), Value::String(id.clone()));
      }
      self.update(collection, &id, doc).await?;
      updated += 1;
    }
    Ok(updated)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    if filter.is_none() {
      let resp = self
        .request(reqwest::Method::GET, &format!("{}/_count", collection))
        .send()
        .await
        .map_err(|e| OrmError::Connection(e.to_string()))?;
      if resp.status().as_u16() == 404 {
        return Ok(0);
      }
      if !resp.status().is_success() {
        return Ok(0);
      }
      let body: Value = resp
        .json()
        .await
        .map_err(|e| OrmError::Connection(e.to_string()))?;
      return Ok(body.get("count").and_then(|v| v.as_u64()).unwrap_or(0));
    }
    let documents = self.read_all(collection).await?;
    Ok(
      documents
        .iter()
        .filter(|d| filter.is_some_and(|f| f.matches(d)))
        .count() as u64,
    )
  }

  async fn create_index(&self, collection: &str, _index: &NosqlIndex) -> OrmResult<()> {
    crate::utils::validate_collection_simple(collection)?;
    let resp = self
      .request(reqwest::Method::PUT, collection)
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if resp.status().is_success() || resp.status().as_u16() == 400 {
      Ok(())
    } else {
      let text = resp.text().await.unwrap_or_default();
      Err(OrmError::Query(format!(
        "Elasticsearch create index failed: {}",
        text
      )))
    }
  }

  async fn drop_index(&self, collection: &str, _index_name: &str) -> OrmResult<()> {
    crate::utils::validate_collection_simple(collection)?;
    let resp = self
      .request(reqwest::Method::DELETE, collection)
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if resp.status().is_success() || resp.status().as_u16() == 404 {
      Ok(())
    } else {
      let text = resp.text().await.unwrap_or_default();
      Err(OrmError::Query(format!(
        "Elasticsearch drop index failed: {}",
        text
      )))
    }
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Ok(Vec::new())
  }

  fn capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities::none().with(crate::provider::Capability::RawQueries)
  }

  async fn health_check(&self) -> OrmResult<bool> {
    let resp = self.request(reqwest::Method::GET, "").send().await;
    Ok(resp.map(|r| r.status().is_success()).unwrap_or(false))
  }
}

#[async_trait]
impl SchemaIntrospection for ElasticsearchProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let resp = self
      .request(reqwest::Method::GET, "_cat/indices?format=json")
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if !resp.status().is_success() {
      return Ok(Vec::new());
    }
    let body: Value = resp
      .json()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    let mut result = Vec::new();
    if let Some(arr) = body.as_array() {
      for item in arr {
        let name = item
          .get("index")
          .and_then(|v| v.as_str())
          .unwrap_or("")
          .to_string();
        if name.starts_with('.') {
          continue;
        }
        let count = self.count(&name, None).await.unwrap_or(0);
        result.push(CollectionMeta {
          name,
          document_count: count,
          size_bytes: 0,
          created_at: None,
          updated_at: None,
        });
      }
    }
    Ok(result)
  }

  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema> {
    let documents = self.read_all(collection).await?;
    let mut fields: HashMap<String, FieldInfo> = HashMap::new();
    for doc in &documents {
      if let Some(obj) = doc.as_object() {
        for (name, value) in obj {
          let kind = crate::utils::value_kind(value);
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
    crate::utils::validate_collection_simple(collection)?;
    let count = self.count(collection, None).await?;
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
    Ok(Vec::new())
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    Ok("elasticsearch".to_string())
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    Ok(vec!["elasticsearch".to_string()])
  }
}

#[async_trait]
impl AdminCommands for ElasticsearchProvider {
  async fn execute_raw(&self, query: &str, _params: Vec<Value>) -> OrmResult<RawResult> {
    let resp = self
      .request(reqwest::Method::POST, "_search")
      .body(query.to_string())
      .header("Content-Type", "application/json")
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if !resp.status().is_success() {
      let text = resp.text().await.unwrap_or_default();
      return Err(OrmError::Query(format!(
        "Elasticsearch raw failed: {}",
        text
      )));
    }
    let body: Value = resp
      .json()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    Ok(RawResult {
      columns: vec![],
      rows: vec![vec![body]],
      affected_rows: 0,
      last_insert_id: None,
    })
  }

  async fn create_collection(
    &self,
    collection: &str,
    _schema: Option<CollectionSchema>,
  ) -> OrmResult<()> {
    crate::utils::validate_collection_simple(collection)?;
    let resp = self
      .request(reqwest::Method::PUT, collection)
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if resp.status().is_success() || resp.status().as_u16() == 400 {
      Ok(())
    } else {
      let text = resp.text().await.unwrap_or_default();
      Err(OrmError::Query(format!(
        "Elasticsearch create index failed: {}",
        text
      )))
    }
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    crate::utils::validate_collection_simple(collection)?;
    let resp = self
      .request(reqwest::Method::DELETE, collection)
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if resp.status().is_success() || resp.status().as_u16() == 404 {
      Ok(())
    } else {
      let text = resp.text().await.unwrap_or_default();
      Err(OrmError::Query(format!(
        "Elasticsearch drop index failed: {}",
        text
      )))
    }
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    crate::utils::validate_collection_simple(from)?;
    crate::utils::validate_collection_simple(to)?;
    self.create_collection(to, None).await?;
    let docs = self.read_all(from).await?;
    for doc in &docs {
      let id = kv_common::document_id(doc).unwrap_or_default();
      if id.is_empty() {
        continue;
      }
      let mut d = doc.clone();
      if let Some(obj) = d.as_object_mut() {
        obj.insert("id".to_string(), Value::String(id.clone()));
      }
      let _ = self.insert(to, d).await;
    }
    self.drop_collection(from).await
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    let resp = self
      .request(reqwest::Method::GET, "")
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    let body: Value = resp
      .json()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    Ok(
      body
        .get("version")
        .and_then(|v| v.get("number"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string(),
    )
  }

  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    let started = std::time::Instant::now();
    let resp = self.request(reqwest::Method::GET, "").send().await;
    match resp {
      Ok(r) if r.status().is_success() => Ok(ConnectionHealth {
        healthy: true,
        latency_ms: Some(started.elapsed().as_millis() as u64),
        server_version: None,
        connected_at: None,
        pool_stats: None,
      }),
      _ => Ok(ConnectionHealth {
        healthy: false,
        latency_ms: None,
        server_version: None,
        connected_at: None,
        pool_stats: None,
      }),
    }
  }
}

#[async_trait]
impl TransactionControl for ElasticsearchProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    Err(OrmError::NotSupported(
      "Elasticsearch has no transactions".to_string(),
    ))
  }
  async fn commit_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "Elasticsearch has no transactions".to_string(),
    ))
  }
  async fn rollback_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "Elasticsearch has no transactions".to_string(),
    ))
  }
  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(false)
  }
}

#[cfg(test)]
mod tests;
