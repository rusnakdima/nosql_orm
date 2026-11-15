use crate::providers::kv_common;
// InfluxDB provider — a native time-series backend (feature `influxdb`).
//
// InfluxDB is a time-series store, not a document store, so the mapping is:
//
//   * collection  -> measurement
//   * document id -> tag `id` (tags are indexed, so `WHERE id='..'` is cheap)
//   * document    -> field `doc`, the whole document as one JSON string
//
// Keeping the document in a single string field avoids InfluxDB's field-type
// conflicts (a field is typed once per measurement) and lets a document carry
// arbitrary nested JSON. Filtering, sorting and pagination therefore happen
// in memory after the read, exactly as `RedisProvider` and `EtcdProvider`
// do it.
//
// The one real mismatch with a document store: InfluxDB has no unique key, so
// writing the same id twice APPENDS a second point instead of replacing it.
// Every read therefore goes through `LAST(doc) ... GROUP BY id`, which keeps
// the newest point per id and makes a document store's "one row per id" hold
// again. `count` uses the same dedup so it agrees with `find_many`.
//
// Because there is no unique constraint, `insert` and `update` check
// existence first. That check-then-write is not atomic — etcd's `Txn`
// compare was — so a concurrent writer can still slip a duplicate in. It is
// documented rather than papered over.

use crate::error::{OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::provider::{
  AdminCommands, Capability, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  FieldInfo, IndexInfo, ProviderCapabilities, RawResult, SchemaIntrospection, Storage,
  TransactionControl, TransactionId,
};
use crate::query::Filter;
// (compare_values, generate_id now in kv_common)
use async_trait::async_trait;
use influxdb::{Client, ReadQuery};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};



// A measurement name is embedded in every query, so it is wrapped in double
// quotes with `\` and `"` escaped. Quoting also lets a name carry the `-`
// and `.` characters that InfluxQL would otherwise reject unquoted.
fn quote_measurement(name: &str) -> String {
  format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
}

// InfluxQL string literals escape `\` and `'`.
fn quote_literal(value: &str) -> String {
  format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

// Line-protocol escaping.
//
// The `influxdb` crate escapes `"` in tag values (its `SLASHES` regex
// includes it), but InfluxDB does not unescape that: the quote is stored
// with a literal backslash in front, so a later `WHERE id='a"b'` never
// matches. Measured against InfluxDB 1.8 — a tag written as `a\"b` reads
// back as `a\"b`, while `a"b` reads back as `a"b`.
//
// So tag values escape exactly `\`, `,`, `=` and space, and nothing else.
fn escape_tag(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  for ch in s.chars() {
    match ch {
      '\\' | ',' | '=' | ' ' => {
        out.push('\\');
        out.push(ch);
      }
      _ => out.push(ch),
    }
  }
  out
}

// Measurement names escape `\`, `,` and space.
fn escape_measurement(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  for ch in s.chars() {
    match ch {
      '\\' | ',' | ' ' => {
        out.push('\\');
        out.push(ch);
      }
      _ => out.push(ch),
    }
  }
  out
}

// A field string value is double-quoted, with `\` and `"` escaped inside.
fn escape_field_string(s: &str) -> String {
  let mut out = String::with_capacity(s.len() + 2);
  out.push('"');
  for ch in s.chars() {
    match ch {
      '\\' | '"' => {
        out.push('\\');
        out.push(ch);
      }
      _ => out.push(ch),
    }
  }
  out.push('"');
  out
}

// A ready-to-send line-protocol batch. Implementing `Query` directly is what
// buys the correct tag escaping above; the crate's `WriteQuery` cannot be
// told to leave `"` alone.
struct LineProtocol(String);

impl influxdb::Query for LineProtocol {
  fn build(&self) -> Result<influxdb::ValidQuery, influxdb::Error> {
    Ok(influxdb::ValidQuery::from(self.0.clone()))
  }

  fn build_with_opts(&self, _use_v2: bool) -> Result<influxdb::ValidQuery, influxdb::Error> {
    Ok(influxdb::ValidQuery::from(self.0.clone()))
  }

  fn get_type(&self) -> influxdb::QueryType {
    influxdb::QueryType::WriteQuery("ns".to_string())
  }
}

// Collection names must be non-empty; anything else is safe because every
// use goes through `quote_measurement`/`quote_literal`.
fn validate_collection(collection: &str) -> OrmResult<()> {
  if collection.is_empty() {
    return Err(OrmError::InvalidInput(
      "Collection name cannot be empty".to_string(),
    ));
  }
  Ok(())
}


fn now_nanos() -> u128 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_nanos())
    .unwrap_or(0)
}

// The shape InfluxDB's `/query` endpoint returns. `error` sits at the top
// level for a failed statement, and `series` is absent for a statement that
// selected nothing (or a DDL/DML statement).
#[derive(Debug, Deserialize)]
struct InfluxResponse {
  #[serde(default)]
  results: Vec<InfluxResult>,
  #[serde(default)]
  error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InfluxResult {
  #[serde(default)]
  series: Vec<InfluxSeries>,
  #[serde(default)]
  error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InfluxSeries {
  #[serde(default)]
  tags: HashMap<String, String>,
  #[serde(default)]
  columns: Vec<String>,
  #[serde(default)]
  values: Vec<Vec<Value>>,
}

fn parse_response(body: &str) -> OrmResult<Vec<InfluxSeries>> {
  let parsed: InfluxResponse =
    serde_json::from_str(body).map_err(|e| OrmError::Serialization(e))?;

  if let Some(err) = parsed.error {
    return Err(OrmError::Query(err));
  }

  let mut series = Vec::new();
  for result in parsed.results {
    if let Some(err) = result.error {
      return Err(OrmError::Query(err));
    }
    series.extend(result.series);
  }
  Ok(series)
}

// Pull the `doc` field out of a `SELECT LAST(doc) ... GROUP BY id` result.
// Each series carries its id in `tags` and the JSON document as the single
// `last` value.
fn docs_from_last_series(series: Vec<InfluxSeries>) -> OrmResult<Vec<Value>> {
  let mut docs = Vec::new();
  for s in series {
    let id = s.tags.get("id").cloned();
    let last_idx = s.columns.iter().position(|c| c == "last").unwrap_or(1);

    for row in &s.values {
      let Some(raw) = row.get(last_idx) else {
        continue;
      };
      let text = match raw {
        Value::String(t) => t.clone(),
        other => other.to_string(),
      };
      let mut doc: Value =
        serde_json::from_str(&text).map_err(|e| OrmError::Serialization(e))?;
      // The tag is authoritative for identity: a document written before an
      // `id` field existed still reads back under the right key.
      if let (Some(id), Some(obj)) = (id.as_ref(), doc.as_object_mut()) {
        obj.insert("id".to_string(), Value::String(id.clone()));
      }
      docs.push(doc);
    }
  }
  Ok(docs)
}

#[derive(Clone)]
pub struct InfluxDbProvider {
  client: Client,
}

impl InfluxDbProvider {
  // `url` is the InfluxDB HTTP endpoint, `database` the database to work in.
  //   InfluxDbProvider::connect("http://127.0.0.1:8086", "mydb").await?
  //
  // Unlike the SQL providers, the database is created if it is missing:
  // `CREATE DATABASE` is idempotent in InfluxDB and a database is a cheap
  // namespace rather than a provisioned resource.
  pub async fn connect(url: impl AsRef<str>, database: impl AsRef<str>) -> OrmResult<Self> {
    let database = database.as_ref();
    let client = Client::new(url.as_ref(), database);

    // Ping first so a bad URL fails here rather than on the CREATE below.
    client
      .ping()
      .await
      .map_err(|e| OrmError::Connection(format!("InfluxDB ping failed: {}", e)))?;

    client
      .query(ReadQuery::new(format!(
        "CREATE DATABASE {}",
        quote_measurement(database)
      )))
      .await
      .map_err(|e| OrmError::Connection(format!("InfluxDB CREATE DATABASE failed: {}", e)))?;

    Ok(Self { client })
  }

  pub async fn from_config(config: &crate::provider::ProviderConfig) -> OrmResult<Self> {
    let database = config
      .database
      .clone()
      .unwrap_or_else(|| "nosql_orm".to_string());

    let mut client = Client::new(&config.connection, &database);
    if let (Some(user), Some(pass)) = (
      config.options.get("username"),
      config.options.get("password"),
    ) {
      client = client.with_auth(user, pass);
    } else if let Some(token) = config.options.get("token") {
      client = client.with_token(token);
    }

    let provider = Self { client };
    provider
      .client
      .ping()
      .await
      .map_err(|e| OrmError::Connection(format!("InfluxDB ping failed: {}", e)))?;
    Ok(provider)
  }

  // Read every document in a collection, newest point per id.
  async fn read_all(&self, collection: &str) -> OrmResult<Vec<Value>> {
    validate_collection(collection)?;
    let sql = format!(
      "SELECT LAST(doc) FROM {} GROUP BY id",
      quote_measurement(collection)
    );
    let body = self
      .client
      .query(ReadQuery::new(sql))
      .await
      .map_err(|e| OrmError::Query(format!("InfluxDB query failed: {}", e)))?;
    docs_from_last_series(parse_response(&body)?)
  }

  async fn write_doc(&self, collection: &str, id: &str, doc: &Value) -> OrmResult<()> {
    validate_collection(collection)?;
    let text = serde_json::to_string(doc)?;

    let line = format!(
      "{measurement},{tag_key}={tag_value} {field_key}={field_value} {time}",
      measurement = escape_measurement(collection),
      tag_key = escape_tag("id"),
      tag_value = escape_tag(id),
      field_key = escape_tag("doc"),
      field_value = escape_field_string(&text),
      time = now_nanos(),
    );

    self
      .client
      .query(LineProtocol(line))
      .await
      .map_err(|e| OrmError::Query(format!("InfluxDB write failed: {}", e)))?;
    Ok(())
  }

  async fn exists_id(&self, collection: &str, id: &str) -> OrmResult<bool> {
    validate_collection(collection)?;
    let sql = format!(
      "SELECT LAST(doc) FROM {} WHERE id={} GROUP BY id",
      quote_measurement(collection),
      quote_literal(id)
    );
    let body = self
      .client
      .query(ReadQuery::new(sql))
      .await
      .map_err(|e| OrmError::Query(format!("InfluxDB query failed: {}", e)))?;
    Ok(!parse_response(&body)?.is_empty())
  }
}

#[async_trait]
impl Storage for InfluxDbProvider {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    validate_collection(collection)?;

    let id = kv_common::document_id_or_new(&doc);
    let mut doc_with_id = doc;
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("id".to_string(), Value::String(id.clone()));
    }

    // No unique constraint exists, so existence is checked first. See the
    // module comment: this is a check-then-write, not an atomic compare.
    if self.exists_id(collection, &id).await? {
      return Err(OrmError::Duplicate(format!(
        "{}/{} already exists",
        collection, id
      )));
    }

    self.write_doc(collection, &id, &doc_with_id).await?;
    Ok(doc_with_id)
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    validate_collection(collection)?;
    let sql = format!(
      "SELECT LAST(doc) FROM {} WHERE id={} GROUP BY id",
      quote_measurement(collection),
      quote_literal(id)
    );
    let body = self
      .client
      .query(ReadQuery::new(sql))
      .await
      .map_err(|e| OrmError::Query(format!("InfluxDB query failed: {}", e)))?;
    let docs = docs_from_last_series(parse_response(&body)?)?;
    Ok(docs.into_iter().next())
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
    let documents = self.read_all(collection).await?;
    Ok(kv_common::select_documents(
      documents, filter, skip, limit, sort_by, sort_asc,
    ))
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    validate_collection(collection)?;

    if !self.exists_id(collection, id).await? {
      return Err(OrmError::NotFound(format!("{}/{}", collection, id)));
    }

    let mut doc_with_id = doc;
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("id".to_string(), Value::String(id.to_string()));
    }

    self.write_doc(collection, id, &doc_with_id).await?;
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

    // `DELETE` reports nothing about how many points it removed, so the
    // existence check comes first to answer `true`/`false` honestly.
    let existed = self.exists_id(collection, id).await?;
    if !existed {
      return Ok(false);
    }

    let sql = format!(
      "DELETE FROM {} WHERE id={}",
      quote_measurement(collection),
      quote_literal(id)
    );
    // The client routes anything without SELECT/SHOW as a POST, which is
    // what InfluxDB requires for DELETE.
    self
      .client
      .query(ReadQuery::new(sql))
      .await
      .map_err(|e| OrmError::Query(format!("InfluxDB delete failed: {}", e)))?;
    Ok(true)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let documents = self.read_all(collection).await?;

    let mut deleted = 0usize;
    for doc in &documents {
      if !filter.as_ref().is_none_or(|f| f.matches(doc)) {
        continue;
      }
      let Some(id) = kv_common::document_id(doc) else {
        continue;
      };
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
      let Some(id) = kv_common::document_id(&doc) else {
        continue;
      };

      if let (Some(updates_obj), Some(doc_obj)) = (updates.as_object(), doc.as_object_mut()) {
        for (key, value) in updates_obj {
          doc_obj.insert(key.clone(), value.clone());
        }
      }

      self.write_doc(collection, &id, &doc).await?;
      updated += 1;
    }
    Ok(updated)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    validate_collection(collection)?;

    // Unfiltered: count distinct ids server-side. The subquery applies the
    // same LAST-per-id dedup the reads use, so the two always agree.
    if filter.is_none() {
      let sql = format!(
        "SELECT COUNT(*) FROM (SELECT LAST(doc) FROM {} GROUP BY id)",
        quote_measurement(collection)
      );
      let body = self
        .client
        .query(ReadQuery::new(sql))
        .await
        .map_err(|e| OrmError::Query(format!("InfluxDB count failed: {}", e)))?;
      let series = parse_response(&body)?;

      for s in &series {
        let Some(idx) = s.columns.iter().position(|c| c.starts_with("count")) else {
          continue;
        };
        for row in &s.values {
          if let Some(Value::Number(n)) = row.get(idx) {
            return Ok(n.as_u64().unwrap_or(0));
          }
        }
      }
      return Ok(0);
    }

    // Filtered: InfluxQL cannot express the generic filter model, so the
    // documents are read and counted in memory, as the other KV providers do.
    let documents = self.read_all(collection).await?;
    Ok(
      documents
        .iter()
        .filter(|doc| filter.is_some_and(|f| f.matches(doc)))
        .count() as u64,
    )
  }

  async fn create_index(&self, _collection: &str, _index: &NosqlIndex) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "InfluxDB indexes its own series; measurements take no user indexes".to_string(),
    ))
  }

  async fn drop_index(&self, _collection: &str, _index_name: &str) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "InfluxDB indexes its own series; measurements take no user indexes".to_string(),
    ))
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Err(OrmError::NotSupported(
      "InfluxDB indexes its own series; measurements take no user indexes".to_string(),
    ))
  }

  fn capabilities(&self) -> ProviderCapabilities {
    // RawQueries is advertised because `execute_raw` really does run
    // InfluxQL. Aggregation stays off: `aggregate` takes a Mongo-style
    // pipeline, which InfluxQL cannot execute.
    ProviderCapabilities::none().with(Capability::RawQueries)
  }

  async fn health_check(&self) -> OrmResult<bool> {
    Ok(self.client.ping().await.is_ok())
  }
}

#[async_trait]
impl SchemaIntrospection for InfluxDbProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let body = self
      .client
      .query(ReadQuery::new("SHOW MEASUREMENTS"))
      .await
      .map_err(|e| OrmError::Query(format!("InfluxDB SHOW failed: {}", e)))?;
    let series = parse_response(&body)?;

    let mut names = Vec::new();
    for s in &series {
      let name_idx = s.columns.iter().position(|c| c == "name").unwrap_or(0);
      for row in &s.values {
        if let Some(Value::String(name)) = row.get(name_idx) {
          names.push(name.clone());
        }
      }
    }
    names.sort();

    let mut result = Vec::with_capacity(names.len());
    for name in names {
      let document_count = self.count(&name, None).await.unwrap_or(0);
      result.push(CollectionMeta {
        name,
        document_count,
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
    let document_count = self.count(collection, None).await?;
    Ok(CollectionStats {
      name: collection.to_string(),
      document_count,
      size_bytes: 0,
      storage_size_bytes: 0,
      index_count: 0,
      index_size_bytes: 0,
      average_document_size: 0,
    })
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Err(OrmError::NotSupported(
      "InfluxDB indexes its own series; measurements take no user indexes".to_string(),
    ))
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    // `Client` keeps the database it was built with and exposes it.
    Ok(self.client.database_name().to_string())
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    let body = self
      .client
      .query(ReadQuery::new("SHOW DATABASES"))
      .await
      .map_err(|e| OrmError::Query(format!("InfluxDB SHOW failed: {}", e)))?;
    let series = parse_response(&body)?;

    let mut names = Vec::new();
    for s in &series {
      let name_idx = s.columns.iter().position(|c| c == "name").unwrap_or(0);
      for row in &s.values {
        if let Some(Value::String(name)) = row.get(name_idx) {
          names.push(name.clone());
        }
      }
    }
    Ok(names)
  }
}

#[async_trait]
impl AdminCommands for InfluxDbProvider {
  // InfluxQL is a real query language, so raw statements are passed through
  // and the response shape mapped onto `RawResult`.
  async fn execute_raw(&self, sql: &str, _params: Vec<Value>) -> OrmResult<RawResult> {
    let body = self
      .client
      .query(ReadQuery::new(sql))
      .await
      .map_err(|e| OrmError::Query(format!("InfluxDB query failed: {}", e)))?;
    let series = parse_response(&body)?;

    if series.is_empty() {
      // A DDL/DML statement selects nothing: no columns, no rows.
      return Ok(RawResult {
        columns: vec![],
        rows: vec![],
        affected_rows: 0,
        last_insert_id: None,
      });
    }

    // `GROUP BY` emits one series per tag set, so rows are gathered from
    // every series — reading only the first would silently drop all but one
    // group. All series of one statement share the same column shape.
    let columns = series[0].columns.clone();
    let mut rows: Vec<Vec<Value>> = Vec::new();
    for s in &series {
      rows.extend(s.values.iter().cloned());
    }
    let affected_rows = rows.len() as u64;

    Ok(RawResult {
      columns,
      rows,
      affected_rows,
      last_insert_id: None,
    })
  }

  async fn create_collection(
    &self,
    collection: &str,
    _schema: Option<CollectionSchema>,
  ) -> OrmResult<()> {
    // A measurement is created by its first write, so there is nothing to
    // create up front — validating the name is the useful part.
    validate_collection(collection)
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    validate_collection(collection)?;
    let sql = format!("DROP MEASUREMENT {}", quote_measurement(collection));
    // POST, which InfluxDB requires for DROP (a GET is read-only).
    self
      .client
      .query(ReadQuery::new(sql))
      .await
      .map_err(|e| OrmError::Query(format!("InfluxDB DROP failed: {}", e)))?;
    Ok(())
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    validate_collection(from)?;
    validate_collection(to)?;

    // InfluxDB has no RENAME, so copy then drop. The copy reuses the same
    // write path, which re-stamps each point with a fresh timestamp.
    let documents = self.read_all(from).await?;
    for doc in &documents {
      let Some(id) = kv_common::document_id(doc) else {
        continue;
      };
      self.write_doc(to, &id, doc).await?;
    }

    self.drop_collection(from).await
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    let (_build, version) = self
      .client
      .ping()
      .await
      .map_err(|e| OrmError::Connection(format!("InfluxDB ping failed: {}", e)))?;
    Ok(version)
  }

  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    let started = std::time::Instant::now();
    match self.client.ping().await {
      Ok((_build, version)) => Ok(ConnectionHealth {
        healthy: true,
        latency_ms: Some(started.elapsed().as_millis() as u64),
        server_version: Some(version),
        connected_at: None,
        pool_stats: None,
      }),
      Err(e) => {
        tracing::warn!("InfluxDB health check failed: {}", e);
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

// InfluxDB has no transaction spanning requests — every write is applied on
// arrival — so there is nothing to begin, commit or roll back.
#[async_trait]
impl TransactionControl for InfluxDbProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    Err(OrmError::NotSupported(
      "the InfluxDB provider has no transaction session".to_string(),
    ))
  }

  async fn commit_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the InfluxDB provider has no transaction session".to_string(),
    ))
  }

  async fn rollback_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the InfluxDB provider has no transaction session".to_string(),
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
