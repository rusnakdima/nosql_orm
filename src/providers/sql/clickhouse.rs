// ClickHouse provider on the official `clickhouse` HTTP crate.
//
// ClickHouse speaks SQL over HTTP without prepared statements, so the crate
// binds `?` placeholders client-side and every statement here goes through
// `Query::bind`. Rows come back as one `JSON` document (meta + data), which
// is parsed into the JSON objects the shared `Storage` body expects.
//
// ClickHouse is not a row store with transactions: UPDATE is a mutation,
// indexes are skip indexes, and there is no interactive transaction. The
// translations below say so in code instead of pretending otherwise.

use crate::error::{OrmError, OrmResult};
use crate::provider::{
  AdminCommands, Capability, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  FieldInfo, IndexInfo, ProviderCapabilities, ProviderConfig, RawResult, SchemaIntrospection,
  Storage, TransactionControl, TransactionId,
};
use crate::providers::sql::common::SqlProviderHelpers;
use crate::providers::sql::utils::{statement_kind, value_as_u64, StatementKind};
use crate::providers::sql::SqlExecutor;
use crate::query::{Filter, OrderBy, Projection};
use crate::sql::introspection::TableIntrospector;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone)]
pub struct ClickHouseProvider {
  client: clickhouse::Client,
  dialect: SqlDialect,
  query_builder: SqlQueryBuilder,
}

impl ClickHouseProvider {
  // The URL may carry credentials (`http://user:secret@host:8123`); the
  // crate takes them as separate calls, so they are split off here.
  pub async fn connect(url: impl AsRef<str>) -> OrmResult<Self> {
    let (clean_url, user, password) = split_userinfo(url.as_ref());
    let mut client = clickhouse::Client::default().with_url(clean_url);
    if let Some(user) = user {
      client = client.with_user(user);
    }
    if let Some(password) = password {
      client = client.with_password(password);
    }

    Ok(Self {
      client,
      // Backtick quoting and `?` placeholders are what ClickHouse accepts
      // and what this crate's binder produces.
      dialect: SqlDialect::MySQL,
      query_builder: SqlQueryBuilder::new(SqlDialect::MySQL),
    })
  }

  pub async fn from_config(config: &ProviderConfig) -> OrmResult<Self> {
    let mut provider = Self::connect(&config.connection).await?;
    if let Some(database) = config.database.as_deref() {
      provider.client = provider.client.with_database(database);
    }
    if let Some(user) = config.options.get("user") {
      provider.client = provider.client.with_user(user.clone());
    }
    if let Some(password) = config.options.get("password") {
      provider.client = provider.client.with_password(password.clone());
    }
    Ok(provider)
  }

  pub fn dialect(&self) -> SqlDialect {
    self.dialect
  }

  pub fn query_builder(&self) -> &SqlQueryBuilder {
    &self.query_builder
  }

  pub async fn build_select(
    &self,
    table: &str,
    filter: Option<&Filter>,
    projection: Option<&Projection>,
    order_by: Option<&[OrderBy]>,
    limit: Option<u32>,
    offset: Option<u64>,
  ) -> OrmResult<String> {
    crate::sql::introspection::build_select(
      self,
      &self.query_builder,
      table,
      filter,
      projection,
      order_by,
      limit,
      offset,
    )
    .await
  }

  // The read path: bind, request the `JSON` document, split it into column
  // names and row objects.
  async fn query_rows(
    &self,
    sql: &str,
    params: Vec<Value>,
  ) -> OrmResult<(Vec<String>, Vec<Value>)> {
    let sql = escape_literal_question_marks(sql);
    let mut query = self.client.query(&sql);
    for param in params {
      query = query.bind(ChParam(&param));
    }
    let mut cursor = query
      .fetch_bytes("JSON")
      .map_err(|e| OrmError::Query(format!("ClickHouse query: {}", e)))?;
    let body = cursor
      .collect()
      .await
      .map_err(|e| OrmError::Query(format!("ClickHouse response: {}", e)))?;
    parse_json_document(&body)
  }

  // ClickHouse reports no affected-row count for the statements the shared
  // `Storage` body runs, so a DELETE or UPDATE is preceded by a count of the
  // rows it matches. A failure there degrades the count, never the write.
  async fn count_matching(&self, sql: &str, params: &[Value]) -> Option<u64> {
    let (table, condition) = mutation_target(sql)?;
    let mut count_sql = format!("SELECT count(*) FROM {}", table);
    if let Some(condition) = condition {
      count_sql.push_str(" WHERE ");
      count_sql.push_str(&condition);
    }
    let (names, rows) = self.query_rows(&count_sql, params.to_vec()).await.ok()?;
    let first = rows.first()?;
    if let Some(value) = first.get(names.first().map(String::as_str).unwrap_or_default()) {
      return value_as_u64(value);
    }
    first.as_object()?.values().next().and_then(value_as_u64)
  }

  async fn write(&self, sql: &str, params: Vec<Value>) -> OrmResult<u64> {
    let kind = statement_kind(sql);
    let matched = if kind == StatementKind::Write {
      self.count_matching(sql, &params).await
    } else {
      None
    };

    let translated = translate_statement(sql)?;
    let sql = escape_literal_question_marks(&translated);
    let mut query = self.client.query(&sql);
    for param in &params {
      query = query.bind(ChParam(param));
    }
    if kind == StatementKind::Write {
      // Mutations run asynchronously unless asked to wait, and the summary
      // header only carries final numbers once the query has finished.
      query = query
        .with_setting("mutations_sync", "2")
        .with_setting("wait_end_of_query", "1");
    }

    let mut cursor = query
      .fetch_bytes("JSON")
      .map_err(|e| OrmError::Query(format!("ClickHouse write: {}", e)))?;
    cursor
      .collect()
      .await
      .map_err(|e| OrmError::Query(format!("ClickHouse write: {}", e)))?;

    if let Some(matched) = matched {
      return Ok(matched);
    }

    let written = cursor.summary().and_then(|s| s.written_rows()).unwrap_or(0);
    Ok(if is_insert_statement(&translated) {
      // An INSERT that reached here succeeded; with no summary the single
      // row it was built for is the honest answer.
      if written == 0 && cursor.summary().is_none() {
        1
      } else {
        written
      }
    } else {
      written
    })
  }
}

// `http://user:secret@host:8123/db` → (url without userinfo, user, password).
fn split_userinfo(url: &str) -> (String, Option<String>, Option<String>) {
  let scheme_end = match url.find("://") {
    Some(index) => index + 3,
    None => return (url.to_string(), None, None),
  };
  let authority_end = url[scheme_end..]
    .find(|c| c == '/' || c == '?')
    .map(|index| scheme_end + index)
    .unwrap_or(url.len());
  let authority = &url[scheme_end..authority_end];
  let at = match authority.rfind('@') {
    Some(index) => index,
    None => return (url.to_string(), None, None),
  };

  let userinfo = &authority[..at];
  let (user, password) = match userinfo.split_once(':') {
    Some((user, password)) => (Some(user.to_string()), Some(password.to_string())),
    None => (Some(userinfo.to_string()), None),
  };

  let mut clean = String::with_capacity(url.len());
  clean.push_str(&url[..scheme_end]);
  clean.push_str(&authority[at + 1..]);
  clean.push_str(&url[authority_end..]);
  (clean, user, password)
}

// The crate's binder treats every `?` it finds as a parameter slot, even one
// inside an inlined string literal, so a literal `?` is doubled to `??` —
// the crate's own escape for a literal question mark.
fn escape_literal_question_marks(sql: &str) -> String {
  let mut out = String::with_capacity(sql.len());
  let mut in_string = false;
  let mut chars = sql.chars().peekable();
  while let Some(ch) = chars.next() {
    if !in_string {
      if ch == '\'' {
        in_string = true;
      }
      out.push(ch);
      continue;
    }
    if ch == '\'' {
      match chars.peek() {
        Some(&'\'') => {
          out.push('\'');
          out.push('\'');
          chars.next();
        }
        _ => {
          in_string = false;
          out.push(ch);
        }
      }
      continue;
    }
    if ch == '?' {
      out.push_str("??");
    } else {
      out.push(ch);
    }
  }
  out
}

// A parameter in the shape the client-side binder wants: a JSON string would
// be written with double quotes, which ClickHouse reads as an identifier.
struct ChParam<'a>(&'a Value);

impl Serialize for ChParam<'_> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    match self.0 {
      Value::Null => serializer.serialize_none(),
      Value::Bool(b) => serializer.serialize_bool(*b),
      Value::Number(n) => {
        if let Some(i) = n.as_i64() {
          serializer.serialize_i64(i)
        } else if let Some(u) = n.as_u64() {
          serializer.serialize_u64(u)
        } else if let Some(f) = n.as_f64() {
          serializer.serialize_f64(f)
        } else {
          serializer.serialize_str(&n.to_string())
        }
      }
      Value::String(s) => serializer.serialize_str(s),
      other => serializer.serialize_str(&serde_json::to_string(other).unwrap_or_default()),
    }
  }
}

// ClickHouse's `JSON` format carries `meta` (ordered column names) and
// `data` (row objects, or arrays when a compact variant is used).
fn parse_json_document(body: &[u8]) -> OrmResult<(Vec<String>, Vec<Value>)> {
  let text = std::str::from_utf8(body)
    .map_err(|e| OrmError::Query(format!("ClickHouse response is not UTF-8: {}", e)))?;
  let parsed: Value = serde_json::from_str(text)
    .map_err(|e| OrmError::Query(format!("ClickHouse response is not JSON: {}", e)))?;

  let names: Vec<String> = parsed
    .get("meta")
    .and_then(Value::as_array)
    .map(|columns| {
      columns
        .iter()
        .filter_map(|column| {
          column
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
        })
        .collect()
    })
    .unwrap_or_default();

  let data = match parsed.get("data").and_then(Value::as_array) {
    Some(data) => data,
    None => return Ok((names, Vec::new())),
  };

  let mut rows = Vec::with_capacity(data.len());
  for entry in data {
    match entry.as_object() {
      Some(_) if names.is_empty() => rows.push(entry.clone()),
      Some(object) => {
        let mut map = serde_json::Map::new();
        for name in &names {
          map.insert(
            name.clone(),
            object.get(name).cloned().unwrap_or(Value::Null),
          );
        }
        rows.push(Value::Object(map));
      }
      None => match entry.as_array() {
        Some(values) => {
          let mut map = serde_json::Map::new();
          for (index, name) in names.iter().enumerate() {
            map.insert(
              name.clone(),
              values.get(index).cloned().unwrap_or(Value::Null),
            );
          }
          rows.push(Value::Object(map));
        }
        None => rows.push(entry.clone()),
      },
    }
  }
  Ok((names, rows))
}

// `UPDATE t SET … WHERE …` is a mutation in ClickHouse: `ALTER TABLE`
// with `UPDATE` where the `SET` keyword sits.
fn rewrite_update(sql: &str) -> Option<String> {
  let trimmed = sql.trim_start();
  let rest = trimmed.strip_prefix("UPDATE ")?;
  let set_at = rest.find(" SET ")?;
  let table = &rest[..set_at];
  let tail = &rest[set_at + " SET ".len()..];
  Some(format!("ALTER TABLE {} UPDATE {}", table, tail))
}

// `CREATE [UNIQUE] INDEX name ON table (cols)` → a skip index. ClickHouse
// skip indexes are not unique, so a UNIQUE request is refused instead of
// silently downgraded.
fn rewrite_create_index(sql: &str) -> OrmResult<Option<String>> {
  let trimmed = sql.trim_start();
  let rest = match trimmed.strip_prefix("CREATE ") {
    Some(rest) => rest,
    None => return Ok(None),
  };
  let (rest, unique) = match rest.strip_prefix("UNIQUE ") {
    Some(rest) => (rest, true),
    None => (rest, false),
  };
  let rest = match rest.strip_prefix("INDEX ") {
    Some(rest) => rest,
    None => return Ok(None),
  };
  let on_at = match rest.find(" ON ") {
    Some(index) => index,
    None => return Ok(None),
  };
  let name = &rest[..on_at];
  let after = &rest[on_at + 4..];
  let open = match after.find(" (") {
    Some(index) => index,
    None => return Ok(None),
  };
  let table = &after[..open];
  let columns = after[open..].trim();

  if unique {
    return Err(OrmError::NotSupported(
      "ClickHouse skip indexes are not unique; a UNIQUE index cannot be created".to_string(),
    ));
  }

  Ok(Some(format!(
    "ALTER TABLE {} ADD INDEX {} {} TYPE minmax GRANULARITY 1",
    table, name, columns
  )))
}

// `DROP INDEX name ON table` (MySQL shape) → `ALTER TABLE … DROP INDEX`.
fn rewrite_drop_index(sql: &str) -> Option<String> {
  let rest = sql.trim_start().strip_prefix("DROP INDEX ")?;
  let on_at = rest.find(" ON ")?;
  let name = &rest[..on_at];
  let table = &rest[on_at + 4..];
  Some(format!("ALTER TABLE {} DROP INDEX {}", table, name))
}

// The shared DDL is `CREATE TABLE x (id TEXT PRIMARY KEY)`. ClickHouse needs
// a table engine and has no TEXT/PRIMARY KEY column constraint, so the
// statement is translated: the SQL types become ClickHouse types and the
// table gets a MergeTree engine.
fn rewrite_create_table(sql: &str) -> Option<String> {
  let rest = sql.trim_start().strip_prefix("CREATE TABLE ")?;
  let (if_not_exists, rest) = match rest.strip_prefix("IF NOT EXISTS ") {
    Some(tail) => ("IF NOT EXISTS ", tail),
    None => ("", rest),
  };
  let open = rest.find(" (")?;
  let table = &rest[..open];
  let close = rest.rfind(')')?;
  if close < open {
    return None;
  }
  let columns = map_column_types(&rest[open + 2..close]);
  Some(format!(
    "CREATE TABLE {}{} ({}) ENGINE = MergeTree ORDER BY tuple()",
    if_not_exists,
    table,
    columns.trim()
  ))
}

fn map_column_types(columns: &str) -> String {
  let conversions = [
    ("VARCHAR", "String"),
    ("TEXT", "String"),
    ("JSON", "String"),
    ("BIGINT", "Int64"),
    ("INTEGER", "Int32"),
    ("BOOLEAN", "UInt8"),
    ("TIMESTAMP", "DateTime"),
    ("DATETIME", "DateTime"),
    ("DOUBLE", "Float64"),
    ("FLOAT", "Float64"),
    ("PRIMARY KEY", ""),
  ];
  let mut out = columns.to_string();
  for (from, to) in conversions {
    out = crate::providers::sql::utils::replace_word(&out, from, to);
  }
  out
}

fn translate_statement(sql: &str) -> OrmResult<String> {
  if let Some(translated) = rewrite_update(sql) {
    return Ok(translated);
  }
  if let Some(translated) = rewrite_create_index(sql)? {
    return Ok(translated);
  }
  if let Some(translated) = rewrite_drop_index(sql) {
    return Ok(translated);
  }
  if let Some(translated) = rewrite_create_table(sql) {
    return Ok(translated);
  }
  Ok(sql.to_string())
}

// The table and the optional WHERE of a DELETE/UPDATE, so the rows it
// touches can be counted before the write runs.
fn mutation_target(sql: &str) -> Option<(String, Option<String>)> {
  let trimmed = sql.trim_start();
  if let Some(rest) = trimmed.strip_prefix("DELETE FROM ") {
    return match rest.find(" WHERE ") {
      Some(index) => Some((
        rest[..index].to_string(),
        Some(rest[index + " WHERE ".len()..].to_string()),
      )),
      None => Some((rest.to_string(), None)),
    };
  }
  if let Some(rest) = trimmed.strip_prefix("UPDATE ") {
    let set_at = rest.find(" SET ")?;
    let table = rest[..set_at].to_string();
    let after_set = &rest[set_at + " SET ".len()..];
    let condition = after_set
      .find(" WHERE ")
      .map(|index| after_set[index + " WHERE ".len()..].to_string());
    return Some((table, condition));
  }
  None
}

fn is_insert_statement(sql: &str) -> bool {
  sql
    .trim_start()
    .get(..7)
    .map_or(false, |head| head.eq_ignore_ascii_case("INSERT "))
}

impl SqlProviderHelpers for ClickHouseProvider {
  fn dialect(&self) -> SqlDialect {
    self.dialect
  }

  fn query_builder(&self) -> &SqlQueryBuilder {
    &self.query_builder
  }
}

#[async_trait]
impl TableIntrospector for ClickHouseProvider {
  async fn table_columns(&self, table: &str) -> OrmResult<Vec<String>> {
    let sql = "SELECT name FROM system.columns WHERE database = currentDatabase() AND table = ? ORDER BY position";
    let (_names, rows) = self
      .query_rows(sql, vec![Value::String(table.to_string())])
      .await?;
    Ok(
      rows
        .iter()
        .filter_map(|row| row.get("name").and_then(Value::as_str).map(str::to_string))
        .collect(),
    )
  }
}

#[async_trait]
impl crate::providers::sql::SqlExecutor for ClickHouseProvider {
  async fn execute(&self, sql: &str, params: Vec<Value>) -> OrmResult<u64> {
    self.write(sql, params).await
  }

  async fn query(&self, sql: &str, params: Vec<Value>) -> OrmResult<Vec<Value>> {
    let (_names, rows) = self.query_rows(sql, params).await?;
    Ok(rows)
  }

  async fn list_indexes_for(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    let sql = "SELECT * FROM system.indices WHERE database = currentDatabase() AND table = ?";
    let (_names, rows) = self
      .query_rows(sql, vec![Value::String(collection.to_string())])
      .await?;

    let indexes = rows
      .iter()
      .filter_map(|row| {
        let name = row.get("name")?.as_str()?.to_string();
        let index_type = row
          .get("type")
          .and_then(Value::as_str)
          .unwrap_or("minmax")
          .to_string();
        let expr = row
          .get("expr")
          .and_then(Value::as_str)
          .unwrap_or_default()
          .trim_matches(|c| c == '(' || c == ')')
          .to_string();
        let fields = expr
          .split(',')
          .map(str::trim)
          .filter(|field| !field.is_empty())
          .map(str::to_string)
          .collect();
        let unique = row
          .get("is_unique")
          .and_then(Value::as_bool)
          .unwrap_or(false);
        Some(IndexInfo {
          name,
          collection: collection.to_string(),
          fields,
          index_type,
          unique,
          sparse: true,
        })
      })
      .collect();

    Ok(indexes)
  }

  fn sql_capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities::none()
      .with(Capability::Indexes)
      .with(Capability::RawQueries)
  }
}

#[async_trait]
impl SchemaIntrospection for ClickHouseProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let sql = "SELECT name FROM system.tables WHERE database = currentDatabase() AND is_temporary = false ORDER BY name";
    let (_names, rows) = self.query_rows(sql, Vec::new()).await?;

    let mut collections = Vec::new();
    for row in rows {
      let name = row
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
      if name.is_empty() {
        continue;
      }
      let count = self.count(&name, None).await.unwrap_or(0);
      collections.push(CollectionMeta {
        name,
        document_count: count,
        size_bytes: 0,
        created_at: None,
        updated_at: None,
      });
    }
    Ok(collections)
  }

  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema> {
    let sql = "SELECT name, type, default_kind, default_expression FROM system.columns WHERE database = currentDatabase() AND table = ? ORDER BY position";
    let (_names, rows) = self
      .query_rows(sql, vec![Value::String(collection.to_string())])
      .await?;

    let mut fields = HashMap::new();
    for row in rows {
      let name = row
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
      if name.is_empty() {
        continue;
      }
      let field_type = row
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("String")
        .to_string();
      let nullable = field_type.contains("Nullable");
      let default_value = row
        .get("default_expression")
        .cloned()
        .filter(|value| !value.is_null());
      fields.insert(
        name.clone(),
        FieldInfo {
          name,
          field_type,
          nullable,
          default_value,
        },
      );
    }
    Ok(CollectionSchema {
      name: collection.to_string(),
      fields,
      indexes: vec![],
      options: Default::default(),
    })
  }

  async fn get_collection_stats(&self, collection: &str) -> OrmResult<CollectionStats> {
    let count = self.count(collection, None).await?;
    // Part sizes live in `system.parts`; a table with no active parts (or a
    // permission error) must not fail the stats call.
    let sql = "SELECT sum(bytes_on_disk) AS size FROM system.parts WHERE database = currentDatabase() AND table = ? AND active";
    let size = self
      .query_rows(sql, vec![Value::String(collection.to_string())])
      .await
      .ok()
      .and_then(|(_names, rows)| rows.first().cloned())
      .and_then(|row| {
        row
          .get("size")
          .cloned()
          .or_else(|| row.as_object().and_then(|map| map.values().next()).cloned())
      })
      .as_ref()
      .and_then(value_as_u64)
      .unwrap_or(0);

    Ok(CollectionStats {
      name: collection.to_string(),
      document_count: count,
      size_bytes: size,
      storage_size_bytes: size,
      index_count: 0,
      index_size_bytes: 0,
      average_document_size: if count > 0 { size / count } else { 0 },
    })
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    self.list_indexes_for(collection).await
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    let (_names, rows) = self
      .query_rows("SELECT currentDatabase()", Vec::new())
      .await?;
    Ok(
      rows
        .first()
        .and_then(|row| {
          row
            .get("currentDatabase")
            .or_else(|| row.as_object().and_then(|map| map.values().next()))
        })
        .and_then(Value::as_str)
        .unwrap_or("default")
        .to_string(),
    )
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    let sql = "SELECT name FROM system.databases WHERE name NOT IN ('system', 'information_schema', 'INFORMATION_SCHEMA') ORDER BY name";
    let (_names, rows) = self.query_rows(sql, Vec::new()).await?;
    Ok(
      rows
        .iter()
        .filter_map(|row| row.get("name").and_then(Value::as_str).map(str::to_string))
        .collect(),
    )
  }
}

#[async_trait]
impl AdminCommands for ClickHouseProvider {
  async fn execute_raw(&self, query: &str, params: Vec<Value>) -> OrmResult<RawResult> {
    if statement_kind(query) == StatementKind::Write {
      let affected_rows = self.execute(query, params).await?;
      return Ok(RawResult {
        columns: vec![],
        rows: vec![],
        affected_rows,
        last_insert_id: None,
      });
    }

    let (names, rows) = self.query_rows(query, params).await?;
    let rows_data: Vec<Vec<Value>> = rows
      .iter()
      .map(|row| {
        if names.is_empty() {
          return row
            .as_object()
            .map(|map| map.values().cloned().collect())
            .unwrap_or_default();
        }
        names
          .iter()
          .map(|name| row.get(name).cloned().unwrap_or(Value::Null))
          .collect()
      })
      .collect();

    Ok(RawResult {
      affected_rows: rows_data.len() as u64,
      columns: names,
      rows: rows_data,
      last_insert_id: None,
    })
  }

  async fn create_collection(
    &self,
    collection: &str,
    _schema: Option<CollectionSchema>,
  ) -> OrmResult<()> {
    let sql = self.build_create_table_sql(collection);
    self.execute(&sql, Vec::new()).await?;
    Ok(())
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    let sql = self.build_drop_table_sql(collection);
    self.execute(&sql, Vec::new()).await?;
    Ok(())
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    let sql = self.build_rename_table_sql(from, to);
    self.execute(&sql, Vec::new()).await?;
    Ok(())
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    let (_names, rows) = self.query_rows("SELECT version()", Vec::new()).await?;
    Ok(
      rows
        .first()
        .and_then(|row| {
          row
            .get("version")
            .or_else(|| row.as_object().and_then(|map| map.values().next()))
        })
        .and_then(Value::as_str)
        .unwrap_or("clickhouse")
        .to_string(),
    )
  }

  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    let server_version = self.get_server_version().await?;
    Ok(ConnectionHealth {
      healthy: true,
      latency_ms: None,
      server_version: Some(server_version),
      connected_at: None,
      pool_stats: None,
    })
  }
}

// ClickHouse has no interactive transactions: mutations commit on their own.
// The trait is implemented so the provider satisfies the same contract as
// every other backend, and it reports that honestly instead of faking state.
#[async_trait]
impl TransactionControl for ClickHouseProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    Err(OrmError::NotSupported(
      "ClickHouse has no interactive transactions; mutations commit on their own".to_string(),
    ))
  }

  async fn commit_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "ClickHouse has no interactive transactions; mutations commit on their own".to_string(),
    ))
  }

  async fn rollback_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "ClickHouse has no interactive transactions; mutations commit on their own".to_string(),
    ))
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(false)
  }
}

#[cfg(test)]
mod tests;
