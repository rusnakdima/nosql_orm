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

fn parse_base(input: &str) -> (String, Option<(String, String)>) {
  if let Ok(url) = reqwest::Url::parse(input) {
    let user = url.username().to_string();
    let pass = url.password().unwrap_or("").to_string();
    let has = !user.is_empty();
    let mut clean = url.clone();
    let _ = clean.set_username("");
    let _ = clean.set_password(None);
    let base = clean.to_string().trim_end_matches('/').to_string();
    if has {
      (base, Some((user, pass)))
    } else {
      (base, None)
    }
  } else {
    (input.trim_end_matches('/').to_string(), None)
  }
}

#[derive(Clone)]
pub struct CouchDbProvider {
  base: String,
  creds: Option<(String, String)>,
  client: Client,
  options_user: Option<String>,
  options_pass: Option<String>,
}

impl CouchDbProvider {
  pub async fn connect(base_url: impl AsRef<str>) -> OrmResult<Self> {
    let (base, creds) = parse_base(base_url.as_ref());
    Ok(Self {
      base,
      creds,
      client: Client::new(),
      options_user: None,
      options_pass: None,
    })
  }

  pub async fn new(connection: impl AsRef<str>) -> OrmResult<Self> {
    Self::connect(connection).await
  }

  pub async fn from_config(config: &crate::provider::ProviderConfig) -> OrmResult<Self> {
    let (base, creds) = parse_base(&config.connection);
    let mut provider = Self {
      base,
      creds,
      client: Client::new(),
      options_user: None,
      options_pass: None,
    };
    if let Some(u) = config.options.get("username") {
      provider.options_user = Some(u.clone());
    }
    if let Some(p) = config.options.get("password") {
      provider.options_pass = Some(p.clone());
    }
    Ok(provider)
  }

  fn creds(&self) -> Option<(String, String)> {
    if let (Some(u), Some(p)) = (&self.options_user, &self.options_pass) {
      Some((u.clone(), p.clone()))
    } else {
      self.creds.clone()
    }
  }

  fn url(&self, path: &str) -> String {
    format!("{}/{}", self.base, path.trim_start_matches('/'))
  }

  fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
    let mut rb = self.client.request(method, self.url(path));
    if let Some((u, p)) = self.creds() {
      rb = rb.basic_auth(u, Some(p));
    }
    rb
  }

  async fn ensure_db(&self, db: &str) -> OrmResult<()> {
    crate::utils::validate_collection_simple(db)?;
    let resp = self
      .request(reqwest::Method::PUT, db)
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    let status = resp.status();
    if status.is_success() || status.as_u16() == 412 {
      Ok(())
    } else {
      let text = resp.text().await.unwrap_or_default();
      Err(OrmError::Query(format!(
        "CouchDB create db failed: {}",
        text
      )))
    }
  }

  async fn read_all(&self, collection: &str) -> OrmResult<Vec<Value>> {
    crate::utils::validate_collection_simple(collection)?;
    let resp = self
      .request(reqwest::Method::GET, &format!("{}/_all_docs", collection))
      .query(&[("include_docs", "true")])
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if resp.status().as_u16() == 404 {
      return Ok(Vec::new());
    }
    if !resp.status().is_success() {
      let text = resp.text().await.unwrap_or_default();
      return Err(OrmError::Query(format!(
        "CouchDB _all_docs failed: {}",
        text
      )));
    }
    let body: Value = resp
      .json()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    let mut docs = Vec::new();
    if let Some(rows) = body.get("rows").and_then(|v| v.as_array()) {
      for row in rows {
        if let Some(doc) = row.get("doc") {
          if doc
            .get("_id")
            .and_then(|v| v.as_str())
            .map(|s| s.starts_with('_'))
            .unwrap_or(false)
          {
            continue;
          }
          docs.push(doc.clone());
        }
      }
    }
    Ok(docs)
  }
}

#[async_trait]
impl Storage for CouchDbProvider {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    crate::utils::validate_collection_simple(collection)?;
    self.ensure_db(collection).await?;

    let id = kv_common::document_id_or_new(&doc);
    let mut doc_with_id = doc;
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("_id".to_string(), Value::String(id.clone()));
      obj.insert("id".to_string(), Value::String(id.clone()));
    }

    if self.find_by_id(collection, &id).await?.is_some() {
      return Err(OrmError::Duplicate(format!(
        "{}/{} already exists",
        collection, id
      )));
    }

    let resp = self
      .request(reqwest::Method::PUT, &format!("{}/{}", collection, id))
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
      return Err(OrmError::Query(format!("CouchDB insert failed: {}", text)));
    }
    let body: Value = resp
      .json()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if let Some(rev) = body.get("rev").and_then(|v| v.as_str()) {
      if let Some(obj) = doc_with_id.as_object_mut() {
        obj.insert("_rev".to_string(), Value::String(rev.to_string()));
      }
    }
    Ok(doc_with_id)
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    crate::utils::validate_collection_simple(collection)?;
    let resp = self
      .request(reqwest::Method::GET, &format!("{}/{}", collection, id))
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if resp.status().as_u16() == 404 {
      return Ok(None);
    }
    if !resp.status().is_success() {
      let text = resp.text().await.unwrap_or_default();
      return Err(OrmError::Query(format!("CouchDB get failed: {}", text)));
    }
    let mut doc: Value = resp
      .json()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if doc.get("id").is_none() {
      if let Some(i) = doc.get("_id").cloned() {
        if let Some(obj) = doc.as_object_mut() {
          obj.insert("id".to_string(), i);
        }
      }
    }
    Ok(Some(doc))
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
    let existing = self
      .find_by_id(collection, id)
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;

    let mut doc_with_id = doc;
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("id".to_string(), Value::String(id.to_string()));
      obj.insert("_id".to_string(), Value::String(id.to_string()));
      if let Some(rev) = existing.get("_rev").cloned() {
        obj.insert("_rev".to_string(), rev);
      }
    }

    let resp = self
      .request(reqwest::Method::PUT, &format!("{}/{}", collection, id))
      .json(&doc_with_id)
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if !resp.status().is_success() {
      let text = resp.text().await.unwrap_or_default();
      return Err(OrmError::Query(format!("CouchDB update failed: {}", text)));
    }
    let body: Value = resp
      .json()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    if let Some(rev) = body.get("rev").and_then(|v| v.as_str()) {
      if let Some(obj) = doc_with_id.as_object_mut() {
        obj.insert("_rev".to_string(), Value::String(rev.to_string()));
      }
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
      // remove _rev? keep
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
    let existing = match self.find_by_id(collection, id).await? {
      Some(d) => d,
      None => return Ok(false),
    };
    let rev = existing.get("_rev").and_then(|v| v.as_str()).unwrap_or("");
    let resp = self
      .request(reqwest::Method::DELETE, &format!("{}/{}", collection, id))
      .query(&[("rev", rev)])
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    Ok(resp.status().is_success())
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let documents = self.read_all(collection).await?;
    let mut deleted = 0usize;
    for doc in &documents {
      let Some(id) = kv_common::document_id(doc).or_else(|| {
        doc
          .get("_id")
          .and_then(|v| v.as_str())
          .map(|s| s.to_string())
      }) else {
        continue;
      };
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
      let id = kv_common::document_id(&doc).or_else(|| {
        doc
          .get("_id")
          .and_then(|v| v.as_str())
          .map(|s| s.to_string())
      });
      let Some(id) = id else {
        continue;
      };
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
    let documents = self.read_all(collection).await?;
    if filter.is_none() {
      return Ok(documents.len() as u64);
    }
    Ok(
      documents
        .iter()
        .filter(|d| filter.is_some_and(|f| f.matches(d)))
        .count() as u64,
    )
  }

  async fn create_index(&self, _collection: &str, _index: &NosqlIndex) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "CouchDB indexes not implemented".to_string(),
    ))
  }

  async fn drop_index(&self, _collection: &str, _index_name: &str) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "CouchDB indexes not implemented".to_string(),
    ))
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Ok(Vec::new())
  }

  fn capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities::none()
  }

  async fn health_check(&self) -> OrmResult<bool> {
    let resp = self.request(reqwest::Method::GET, "").send().await;
    Ok(resp.map(|r| r.status().is_success()).unwrap_or(false))
  }
}

#[async_trait]
impl SchemaIntrospection for CouchDbProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let resp = self
      .request(reqwest::Method::GET, "_all_dbs")
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
    let mut names = Vec::new();
    if let Some(arr) = body.as_array() {
      for v in arr {
        if let Some(s) = v.as_str() {
          if !s.starts_with('_') {
            names.push(s.to_string());
          }
        }
      }
    }
    let mut result = Vec::new();
    for name in names {
      let count = self.count(&name, None).await.unwrap_or(0);
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

  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema> {
    let documents = self.read_all(collection).await?;
    let mut fields: HashMap<String, FieldInfo> = HashMap::new();
    for doc in &documents {
      if let Some(obj) = doc.as_object() {
        for (name, value) in obj {
          if name.starts_with('_') {
            continue;
          }
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
    Ok("couchdb".to_string())
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    Ok(vec!["couchdb".to_string()])
  }
}

#[async_trait]
impl AdminCommands for CouchDbProvider {
  async fn execute_raw(&self, _query: &str, _params: Vec<Value>) -> OrmResult<RawResult> {
    Err(OrmError::NotSupported(
      "CouchDB has no raw query".to_string(),
    ))
  }

  async fn create_collection(
    &self,
    collection: &str,
    _schema: Option<CollectionSchema>,
  ) -> OrmResult<()> {
    crate::utils::validate_collection_simple(collection)?;
    self.ensure_db(collection).await
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
      Err(OrmError::Query(format!("CouchDB drop db failed: {}", text)))
    }
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    crate::utils::validate_collection_simple(from)?;
    crate::utils::validate_collection_simple(to)?;
    self.ensure_db(to).await?;
    let docs = self.read_all(from).await?;
    for doc in &docs {
      let mut d = doc.clone();
      if let Some(obj) = d.as_object_mut() {
        obj.remove("_rev");
        let id = kv_common::document_id(&d).unwrap_or_default();
        if !id.is_empty() {
          let _ = self.insert(to, d).await;
        }
      }
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
impl TransactionControl for CouchDbProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    Err(OrmError::NotSupported(
      "CouchDB has no transactions".to_string(),
    ))
  }
  async fn commit_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "CouchDB has no transactions".to_string(),
    ))
  }
  async fn rollback_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "CouchDB has no transactions".to_string(),
    ))
  }
  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(false)
  }
}

#[cfg(test)]
mod tests;
