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

fn sha256_hex(data: &[u8]) -> String {
  use ring::digest;
  let d = digest::digest(&digest::SHA256, data);
  hex_encode(d.as_ref())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
  use ring::hmac;
  let k = hmac::Key::new(hmac::HMAC_SHA256, key);
  hmac::sign(&k, data).as_ref().to_vec()
}

fn hex_encode(bytes: &[u8]) -> String {
  const HEX: &[u8] = b"0123456789abcdef";
  let mut out = String::with_capacity(bytes.len() * 2);
  for b in bytes {
    out.push(HEX[(b >> 4) as usize] as char);
    out.push(HEX[(b & 0x0f) as usize] as char);
  }
  out
}

#[derive(Clone)]
pub struct DynamoDbProvider {
  endpoint: String,
  host: String,
  region: String,
  access_key: String,
  secret_key: String,
  session_token: Option<String>,
  client: Client,
}

impl DynamoDbProvider {
  pub async fn new(connection: impl AsRef<str>) -> OrmResult<Self> {
    Self::connect(connection).await
  }

  pub async fn connect_with_credentials(
    endpoint: impl AsRef<str>,
    region: impl AsRef<str>,
    access_key: impl AsRef<str>,
    secret_key: impl AsRef<str>,
  ) -> OrmResult<Self> {
    let endpoint = endpoint.as_ref().trim_end_matches('/').to_string();
    let url = reqwest::Url::parse(&endpoint).map_err(|e| OrmError::Connection(e.to_string()))?;
    let host = match url.port() {
      Some(p) => format!("{}:{}", url.host_str().unwrap_or("localhost"), p),
      None => url.host_str().unwrap_or("localhost").to_string(),
    };
    Ok(Self {
      endpoint,
      host,
      region: region.as_ref().to_string(),
      access_key: access_key.as_ref().to_string(),
      secret_key: secret_key.as_ref().to_string(),
      session_token: None,
      client: Client::new(),
    })
  }

  pub async fn connect(endpoint: impl AsRef<str>) -> OrmResult<Self> {
    Self::connect_with_credentials(endpoint, "us-east-1", "dummy", "dummy").await
  }

  pub async fn from_config(config: &crate::provider::ProviderConfig) -> OrmResult<Self> {
    let region = config
      .options
      .get("region")
      .cloned()
      .unwrap_or_else(|| "us-east-1".to_string());
    let access_key = config
      .options
      .get("access_key")
      .cloned()
      .or_else(|| std::env::var("AWS_ACCESS_KEY_ID").ok())
      .unwrap_or_else(|| "dummy".to_string());
    let secret_key = config
      .options
      .get("secret_key")
      .cloned()
      .or_else(|| std::env::var("AWS_SECRET_ACCESS_KEY").ok())
      .unwrap_or_else(|| "dummy".to_string());
    let mut p =
      Self::connect_with_credentials(&config.connection, &region, &access_key, &secret_key).await?;
    if let Some(tok) = config.options.get("session_token") {
      p.session_token = Some(tok.clone());
    }
    Ok(p)
  }

  fn sign_headers(&self, target: &str, body: &str, timestamp: &str) -> String {
    let date = &timestamp[..8];
    let service = "dynamodb";
    let content_type = "application/x-amz-json-1.0";

    let canonical_uri = "/";
    let canonical_querystring = "";
    let canonical_headers = format!(
      "content-type:{}\nhost:{}\nx-amz-date:{}\nx-amz-target:{}\n",
      content_type, self.host, timestamp, target
    );
    let signed_headers = "content-type;host;x-amz-date;x-amz-target".to_string();
    if self.session_token.is_some() {
      // not included for simplicity
    }
    let payload_hash = sha256_hex(body.as_bytes());

    let canonical_request = format!(
      "POST\n{}\n{}\n{}\n{}\n{}",
      canonical_uri, canonical_querystring, canonical_headers, signed_headers, payload_hash
    );

    let credential_scope = format!("{}/{}/{}/aws4_request", date, self.region, service);
    let string_to_sign = format!(
      "AWS4-HMAC-SHA256\n{}\n{}\n{}",
      timestamp,
      credential_scope,
      sha256_hex(canonical_request.as_bytes())
    );

    let k_date = hmac_sha256(
      format!("AWS4{}", self.secret_key).as_bytes(),
      date.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, self.region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    let signature = hex_encode(&hmac_sha256(&k_signing, string_to_sign.as_bytes()));

    format!(
      "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
      self.access_key, credential_scope, signed_headers, signature
    )
  }

  async fn request(&self, target: &str, body: Value) -> OrmResult<Value> {
    let body_str = serde_json::to_string(&body)?;
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let auth = self.sign_headers(target, &body_str, &timestamp);

    let mut rb = self
      .client
      .post(&self.endpoint)
      .header("Content-Type", "application/x-amz-json-1.0")
      .header("X-Amz-Target", target)
      .header("X-Amz-Date", &timestamp)
      .header("Host", &self.host)
      .header("Authorization", &auth)
      .body(body_str);
    if let Some(tok) = &self.session_token {
      rb = rb.header("X-Amz-Security-Token", tok);
    }

    let resp = rb
      .send()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if text.is_empty() {
      return Ok(serde_json::json!({}));
    }
    let json: Value = serde_json::from_str(&text).unwrap_or(serde_json::json!({"raw": text}));
    if !status.is_success() {
      let err_type = json.get("__type").and_then(|v| v.as_str()).unwrap_or("");
      if err_type.contains("ConditionalCheckFailed") {
        // caller distinguishes by target? return Duplicate generically
        return Err(OrmError::Duplicate("conditional check failed".to_string()));
      }
      if err_type.contains("ResourceNotFoundException") {
        return Err(OrmError::NotFound(text));
      }
      return Err(OrmError::Query(format!("DynamoDB error: {}", text)));
    }
    Ok(json)
  }

  fn item_to_doc(item: &Value) -> Option<Value> {
    let doc_str = item.get("doc")?.get("S")?.as_str()?;
    serde_json::from_str(doc_str).ok()
  }

  async fn ensure_table(&self, table: &str) -> OrmResult<()> {
    crate::utils::validate_collection_simple(table)?;
    let body = serde_json::json!({
      "TableName": table,
      "KeySchema": [{"AttributeName":"id","KeyType":"HASH"}],
      "AttributeDefinitions": [{"AttributeName":"id","AttributeType":"S"}],
      "BillingMode": "PAY_PER_REQUEST"
    });
    match self.request("DynamoDB_20120810.CreateTable", body).await {
      Ok(_) => Ok(()),
      Err(OrmError::Query(msg)) if msg.contains("ResourceInUseException") => Ok(()),
      Err(e) => Err(e),
    }
  }

  async fn scan_all(&self, table: &str) -> OrmResult<Vec<Value>> {
    crate::utils::validate_collection_simple(table)?;
    let mut docs = Vec::new();
    let mut start_key: Option<Value> = None;
    loop {
      let mut body = serde_json::json!({"TableName": table});
      if let Some(k) = &start_key {
        body["ExclusiveStartKey"] = k.clone();
      }
      let resp = self.request("DynamoDB_20120810.Scan", body).await?;
      if let Some(items) = resp.get("Items").and_then(|v| v.as_array()) {
        for item in items {
          if let Some(doc) = Self::item_to_doc(item) {
            docs.push(doc);
          }
        }
      }
      if let Some(lk) = resp.get("LastEvaluatedKey") {
        if lk.is_null() {
          break;
        }
        start_key = Some(lk.clone());
      } else {
        break;
      }
      if start_key.is_none() {
        break;
      }
    }
    Ok(docs)
  }

  async fn get_item(&self, table: &str, id: &str) -> OrmResult<Option<Value>> {
    crate::utils::validate_collection_simple(table)?;
    let body = serde_json::json!({
      "TableName": table,
      "Key": {"id": {"S": id}}
    });
    let resp = self.request("DynamoDB_20120810.GetItem", body).await?;
    if let Some(item) = resp.get("Item") {
      if item.is_null() {
        return Ok(None);
      }
      return Ok(Self::item_to_doc(item));
    }
    Ok(None)
  }

  async fn put_item(&self, table: &str, id: &str, doc: &Value) -> OrmResult<()> {
    crate::utils::validate_collection_simple(table)?;
    let json = serde_json::to_string(doc)?;
    let body = serde_json::json!({
      "TableName": table,
      "Item": {
        "id": {"S": id},
        "doc": {"S": json}
      }
    });
    self.request("DynamoDB_20120810.PutItem", body).await?;
    Ok(())
  }
}

#[async_trait]
impl Storage for DynamoDbProvider {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    crate::utils::validate_collection_simple(collection)?;
    self.ensure_table(collection).await?;

    let id = kv_common::document_id_or_new(&doc);
    let mut doc_with_id = doc;
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("id".to_string(), Value::String(id.clone()));
    }

    if self.get_item(collection, &id).await?.is_some() {
      return Err(OrmError::Duplicate(format!(
        "{}/{} already exists",
        collection, id
      )));
    }

    // conditional insert
    let json = serde_json::to_string(&doc_with_id)?;
    let body = serde_json::json!({
      "TableName": collection,
      "Item": {
        "id": {"S": id},
        "doc": {"S": json}
      },
      "ConditionExpression": "attribute_not_exists(id)"
    });
    match self.request("DynamoDB_20120810.PutItem", body).await {
      Ok(_) => Ok(doc_with_id),
      Err(OrmError::Duplicate(_)) => Err(OrmError::Duplicate(format!(
        "{}/{} already exists",
        collection, id
      ))),
      Err(e) => Err(e),
    }
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    crate::utils::validate_collection_simple(collection)?;
    self.get_item(collection, id).await
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
    let documents = self.scan_all(collection).await.unwrap_or_default();
    Ok(kv_common::select_documents(
      documents, filter, skip, limit, sort_by, sort_asc,
    ))
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    crate::utils::validate_collection_simple(collection)?;
    if self.get_item(collection, id).await?.is_none() {
      return Err(OrmError::NotFound(format!("{}/{}", collection, id)));
    }
    let mut doc_with_id = doc;
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("id".to_string(), Value::String(id.to_string()));
    }
    self.put_item(collection, id, &doc_with_id).await?;
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
    let existed = self.get_item(collection, id).await?.is_some();
    if !existed {
      return Ok(false);
    }
    let body = serde_json::json!({
      "TableName": collection,
      "Key": {"id": {"S": id}}
    });
    self.request("DynamoDB_20120810.DeleteItem", body).await?;
    Ok(true)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let documents = self.scan_all(collection).await?;
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
    let documents = self.scan_all(collection).await?;
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
      self.put_item(collection, &id, &doc).await?;
      updated += 1;
    }
    Ok(updated)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let documents = self.scan_all(collection).await?;
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
      "DynamoDB indexes not implemented".to_string(),
    ))
  }

  async fn drop_index(&self, _collection: &str, _index_name: &str) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "DynamoDB indexes not implemented".to_string(),
    ))
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Ok(Vec::new())
  }

  fn capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities::none()
  }

  async fn health_check(&self) -> OrmResult<bool> {
    // ListTables as health check
    let body = serde_json::json!({});
    Ok(
      self
        .request("DynamoDB_20120810.ListTables", body)
        .await
        .is_ok(),
    )
  }
}

#[async_trait]
impl SchemaIntrospection for DynamoDbProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let body = serde_json::json!({});
    let resp = self.request("DynamoDB_20120810.ListTables", body).await?;
    let mut names = Vec::new();
    if let Some(arr) = resp.get("TableNames").and_then(|v| v.as_array()) {
      for v in arr {
        if let Some(s) = v.as_str() {
          names.push(s.to_string());
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
    let documents = self.scan_all(collection).await?;
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
    Ok("dynamodb".to_string())
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    Ok(vec!["dynamodb".to_string()])
  }
}

#[async_trait]
impl AdminCommands for DynamoDbProvider {
  async fn execute_raw(&self, _query: &str, _params: Vec<Value>) -> OrmResult<RawResult> {
    Err(OrmError::NotSupported(
      "DynamoDB has no raw query".to_string(),
    ))
  }

  async fn create_collection(
    &self,
    collection: &str,
    _schema: Option<CollectionSchema>,
  ) -> OrmResult<()> {
    crate::utils::validate_collection_simple(collection)?;
    self.ensure_table(collection).await
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    crate::utils::validate_collection_simple(collection)?;
    let body = serde_json::json!({"TableName": collection});
    match self.request("DynamoDB_20120810.DeleteTable", body).await {
      Ok(_) => Ok(()),
      Err(OrmError::NotFound(_)) => Ok(()),
      Err(e) => Err(e),
    }
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    crate::utils::validate_collection_simple(from)?;
    crate::utils::validate_collection_simple(to)?;
    self.ensure_table(to).await?;
    let docs = self.scan_all(from).await?;
    for doc in &docs {
      let id = kv_common::document_id(doc).unwrap_or_default();
      if id.is_empty() {
        continue;
      }
      let mut d = doc.clone();
      if let Some(obj) = d.as_object_mut() {
        obj.insert("id".to_string(), Value::String(id.clone()));
      }
      let _ = self.put_item(to, &id, &d).await;
    }
    self.drop_collection(from).await
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    Ok("dynamodb".to_string())
  }

  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    let started = std::time::Instant::now();
    let body = serde_json::json!({});
    match self.request("DynamoDB_20120810.ListTables", body).await {
      Ok(_) => Ok(ConnectionHealth {
        healthy: true,
        latency_ms: Some(started.elapsed().as_millis() as u64),
        server_version: None,
        connected_at: None,
        pool_stats: None,
      }),
      Err(_) => Ok(ConnectionHealth {
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
impl TransactionControl for DynamoDbProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    Err(OrmError::NotSupported(
      "DynamoDB transactions not implemented".to_string(),
    ))
  }
  async fn commit_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "DynamoDB transactions not implemented".to_string(),
    ))
  }
  async fn rollback_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "DynamoDB transactions not implemented".to_string(),
    ))
  }
  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(false)
  }
}

#[cfg(test)]
mod tests;
