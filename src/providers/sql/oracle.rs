// Oracle provider on the native `oracle` crate (rust-oracle).
//
// The driver vendors ODPI-C and loads the Oracle Client library when a
// connection is made, so the provider compiles and links on a machine with
// no Oracle software installed — only `connect` needs Instant Client at
// runtime. The connection is synchronous, so it lives behind a mutex on
// blocking tasks, the same shape as the DuckDB provider.
//
// Oracle differs from the shared SQL in three places and fixes each one here:
// positional binds are `:1, :2` rather than `?`, TRUE/FALSE have no SQL
// meaning (columns are NUMBER(1)), and `DROP … IF EXISTS` only exists from
// 23c on.

use crate::error::{OrmError, OrmResult};
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  FieldInfo, IndexInfo, ProviderConfig, RawResult, SchemaIntrospection,
  Storage, TransactionControl, TransactionId,
};
use crate::providers::sql::common::SqlProviderHelpers;
use crate::providers::sql::utils::{
  rewrite_portable_sql, statement_kind, value_as_u64, PlaceholderStyle, StatementKind,
};
use crate::providers::sql::SqlExecutor;
use crate::query::{Filter, OrderBy, Projection};
use crate::sql::introspection::TableIntrospector;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use async_trait::async_trait;
use oracle::sql_type::OracleType;
use serde_json::Value;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct OracleProvider {
  conn: Arc<tokio::sync::Mutex<oracle::Connection>>,
  dialect: SqlDialect,
  query_builder: SqlQueryBuilder,
  transaction_manager: Arc<tokio::sync::Mutex<Option<TransactionId>>>,
}

impl OracleProvider {
  // The DSN carries the credentials: `user/password@//host:1521/service`
  // (EZConnect) or `user/password@tns_alias`.
  pub async fn connect(dsn: impl AsRef<str>) -> OrmResult<Self> {
    let dsn = dsn.as_ref();
    let (credentials, connect_string) = dsn.rsplit_once('@').ok_or_else(|| {
      OrmError::Connection(
        "Oracle connection string must look like user/password@//host:1521/service".to_string(),
      )
    })?;
    let (user, password) = credentials.split_once('/').ok_or_else(|| {
      OrmError::Connection(
        "Oracle connection string must look like user/password@//host:1521/service".to_string(),
      )
    })?;
    Self::connect_with_credentials(user, password, connect_string).await
  }

  pub async fn connect_with_credentials(
    user: &str,
    password: &str,
    connect_string: &str,
  ) -> OrmResult<Self> {
    let user = user.to_string();
    let password = password.to_string();
    let connect_string = connect_string.to_string();
    let conn = tokio::task::spawn_blocking(move || {
      oracle::Connection::connect(&user, &password, &connect_string).map_err(|e| {
        OrmError::Connection(format!(
          "Oracle connect failed (the Oracle Client library must be installed): {}",
          e
        ))
      })
    })
    .await
    .map_err(|e| OrmError::Connection(format!("Task join error: {}", e)))??;

    Ok(Self {
      conn: Arc::new(tokio::sync::Mutex::new(conn)),
      // Double-quoted identifiers round-trip: the DDL below quotes them too,
      // so case never changes between creating a table and reading from it.
      dialect: SqlDialect::SQLite,
      query_builder: SqlQueryBuilder::new(SqlDialect::SQLite),
      transaction_manager: Arc::new(tokio::sync::Mutex::new(None)),
    })
  }

  pub async fn from_config(config: &ProviderConfig) -> OrmResult<Self> {
    Self::connect(&config.connection).await
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

  async fn with_connection<R, F>(&self, f: F) -> OrmResult<R>
  where
    F: FnOnce(&oracle::Connection) -> Result<R, oracle::Error> + Send + 'static,
    R: Send + 'static,
  {
    let conn = self.conn.clone();
    let result = tokio::task::spawn_blocking(move || {
      let guard = conn.blocking_lock();
      f(&guard)
    })
    .await
    .map_err(|e| OrmError::Connection(format!("Task join error: {}", e)))?;
    result.map_err(|e| OrmError::Provider(format!("Oracle error: {}", e)))
  }

  async fn query_rows(
    &self,
    sql: &str,
    params: Vec<Value>,
  ) -> OrmResult<(Vec<String>, Vec<Value>)> {
    let sql = rewrite_portable_sql(&strip_drop_if_exists(sql), PlaceholderStyle::Oracle);
    self
      .with_connection(move |conn| {
        let owned: Vec<OwnedParam> = params.iter().map(OwnedParam::from_json).collect();
        let refs: Vec<&dyn oracle::sql_type::ToSql> =
          owned.iter().map(OwnedParam::as_to_sql).collect();

        let mut rows = conn.query(&sql, &refs)?;
        let columns: Vec<(String, OracleType)> = rows
          .column_info()
          .iter()
          .map(|info| (info.name().to_string(), info.oracle_type().clone()))
          .collect();

        let mut out = Vec::new();
        while let Some(row) = rows.next() {
          let row = row?;
          out.push(oracle_row_to_json(&row, &columns));
        }
        Ok((columns.iter().map(|(name, _)| name.clone()).collect(), out))
      })
      .await
  }
}

// A bind parameter owned by the statement: the driver takes `&dyn ToSql`
// references that must outlive the call, and the JSON values the ORM carries
// have no Rust type until this conversion.
enum OwnedParam {
  // `Null` carries the Option it binds as, so the reference below points at
  // an owned value instead of a temporary.
  Null(Option<String>),
  Text(String),
  Int(i64),
  Float(f64),
}

impl OwnedParam {
  fn from_json(value: &Value) -> Self {
    match value {
      // Oracle has no SQL BOOLEAN: a bind of `bool` is a PL/SQL-only type,
      // so booleans travel as NUMBER(1) exactly like the literal rewrite.
      Value::Null => OwnedParam::Null(None),
      Value::Bool(b) => OwnedParam::Int(i64::from(*b)),
      Value::Number(n) => {
        if let Some(i) = n.as_i64() {
          OwnedParam::Int(i)
        } else if let Some(u) = n.as_u64() {
          match i64::try_from(u) {
            Ok(i) => OwnedParam::Int(i),
            Err(_) => OwnedParam::Float(n.as_f64().unwrap_or(0.0)),
          }
        } else if let Some(f) = n.as_f64() {
          OwnedParam::Float(f)
        } else {
          OwnedParam::Text(n.to_string())
        }
      }
      Value::String(s) => OwnedParam::Text(s.clone()),
      other => OwnedParam::Text(serde_json::to_string(other).unwrap_or_default()),
    }
  }

  fn as_to_sql(&self) -> &dyn oracle::sql_type::ToSql {
    match self {
      OwnedParam::Null(inner) => inner,
      OwnedParam::Text(text) => text,
      OwnedParam::Int(value) => value,
      OwnedParam::Float(value) => value,
    }
  }
}

// Oracle fetches every column with its type attached, so JSON conversion
// dispatches on that type instead of guessing from the value.
fn oracle_row_to_json(row: &oracle::Row, columns: &[(String, OracleType)]) -> Value {
  let mut map = serde_json::Map::new();
  for (index, (name, oracle_type)) in columns.iter().enumerate() {
    let value = match oracle_type {
      OracleType::Number(..)
      | OracleType::Float(_)
      | OracleType::BinaryFloat
      | OracleType::BinaryDouble
      | OracleType::Int64 => number_to_json(row, index),
      OracleType::Varchar2(_)
      | OracleType::NVarchar2(_)
      | OracleType::Char(_)
      | OracleType::NChar(_)
      | OracleType::CLOB
      | OracleType::NCLOB
      | OracleType::Long
      | OracleType::Json
      | OracleType::Xml
      | OracleType::Rowid => row
        .get::<usize, String>(index)
        .map(|text| serde_json::json!(text))
        .unwrap_or(Value::Null),
      OracleType::Raw(_) | OracleType::BLOB | OracleType::LongRaw | OracleType::BFILE => row
        .get::<usize, Vec<u8>>(index)
        .map(|bytes| Value::String(crate::providers::sql::utils::base64_encode(&bytes)))
        .unwrap_or(Value::Null),
      OracleType::Date
      | OracleType::Timestamp(_)
      | OracleType::TimestampTZ(_)
      | OracleType::TimestampLTZ(_)
      | OracleType::IntervalDS(_, _)
      | OracleType::IntervalYM(_) => row
        .get::<usize, String>(index)
        .map(|text| serde_json::json!(text))
        .unwrap_or(Value::Null),
      _ => oracle_fallback_json(row, index),
    };
    map.insert(name.clone(), value);
  }
  Value::Object(map)
}

// A NUMBER is read as f64: integers keep their value when they fit the
// 2^53 exactly-representable range, and fractions stay fractions instead of
// being truncated by an integer conversion.
fn number_to_json(row: &oracle::Row, index: usize) -> Value {
  match row.get::<usize, f64>(index) {
    Ok(value) => {
      if value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 {
        serde_json::json!(value as i64)
      } else {
        serde_json::json!(value)
      }
    }
    Err(_) => match row.get::<usize, String>(index) {
      Ok(text) => serde_json::json!(text),
      Err(_) => Value::Null,
    },
  }
}

// Types not named above (objects, refs, editions of newer releases) still
// come back as their string shape rather than disappearing.
fn oracle_fallback_json(row: &oracle::Row, index: usize) -> Value {
  if let Ok(text) = row.get::<usize, String>(index) {
    return serde_json::json!(text);
  }
  if let Ok(value) = row.get::<usize, f64>(index) {
    return serde_json::json!(value);
  }
  if let Ok(bytes) = row.get::<usize, Vec<u8>>(index) {
    return Value::String(crate::providers::sql::utils::base64_encode(&bytes));
  }
  Value::Null
}

// Oracle before 23c rejects `DROP TABLE/INDEX … IF EXISTS`; the ORM's shared
// builder emits it, so the clause is dropped on the way out.
fn strip_drop_if_exists(sql: &str) -> String {
  let trimmed = sql.trim_start();
  let is_drop = trimmed.get(..11).map_or(false, |head| {
    head.eq_ignore_ascii_case("DROP TABLE ") || head.eq_ignore_ascii_case("DROP INDEX ")
  });
  if !is_drop {
    return sql.to_string();
  }
  match trimmed.find(" IF EXISTS ") {
    Some(at) => {
      let mut out = String::with_capacity(trimmed.len());
      out.push_str(&trimmed[..at]);
      out.push(' ');
      out.push_str(&trimmed[at + " IF EXISTS ".len()..]);
      out
    }
    None => sql.to_string(),
  }
}

impl SqlProviderHelpers for OracleProvider {
  fn dialect(&self) -> SqlDialect {
    self.dialect
  }

  fn query_builder(&self) -> &SqlQueryBuilder {
    &self.query_builder
  }

  // The shared DDL types the key as TEXT, which Oracle does not have, and
  // leaves `id` unquoted, which Oracle would upper-case against every quoted
  // read that follows. Both are fixed for the statement this provider owns.
  fn build_create_table_sql(&self, collection: &str) -> String {
    format!(
      "CREATE TABLE {} ({} VARCHAR2(4000) PRIMARY KEY)",
      self.quote_identifier(collection),
      self.quote_identifier("id")
    )
  }
}

#[async_trait]
impl TableIntrospector for OracleProvider {
  async fn table_columns(&self, table: &str) -> OrmResult<Vec<String>> {
    let sql = "SELECT column_name FROM user_tab_columns WHERE table_name = ? ORDER BY column_id";
    let (_names, rows) = self
      .query_rows(sql, vec![Value::String(table.to_string())])
      .await?;
    Ok(
      rows
        .iter()
        .filter_map(|row| {
          row
            .get("COLUMN_NAME")
            .and_then(Value::as_str)
            .map(str::to_string)
        })
        .collect(),
    )
  }
}

#[async_trait]
impl crate::providers::sql::SqlExecutor for OracleProvider {
  async fn execute(&self, sql: &str, params: Vec<Value>) -> OrmResult<u64> {
    let sql = rewrite_portable_sql(&strip_drop_if_exists(sql), PlaceholderStyle::Oracle);
    // An explicit transaction owns the commit; outside one every write is
    // committed here, because Oracle keeps statements in an open
    // transaction until something commits them.
    let in_transaction = self.is_transaction_active().await?;
    self
      .with_connection(move |conn| {
        let owned: Vec<OwnedParam> = params.iter().map(OwnedParam::from_json).collect();
        let refs: Vec<&dyn oracle::sql_type::ToSql> =
          owned.iter().map(OwnedParam::as_to_sql).collect();

        let stmt = conn.execute(&sql, &refs)?;
        let affected = stmt.row_count().unwrap_or(0);
        drop(stmt);
        if !in_transaction {
          conn.commit()?;
        }
        Ok(affected)
      })
      .await
  }

  async fn query(&self, sql: &str, params: Vec<Value>) -> OrmResult<Vec<Value>> {
    let (_names, rows) = self.query_rows(sql, params).await?;
    Ok(rows)
  }

  async fn list_indexes_for(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    let sql = "SELECT i.index_name AS index_name, i.uniqueness AS uniqueness, c.column_name AS column_name FROM user_indexes i JOIN user_ind_columns c ON c.index_name = i.index_name WHERE i.table_name = ? ORDER BY i.index_name, c.column_position";
    let (_names, rows) = self
      .query_rows(sql, vec![Value::String(collection.to_string())])
      .await?;

    let mut order: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, IndexInfo> = HashMap::new();
    for row in rows {
      let name = match row.get("INDEX_NAME").and_then(Value::as_str) {
        Some(name) => name.to_string(),
        None => continue,
      };
      let unique = row.get("UNIQUENESS").and_then(Value::as_str) == Some("UNIQUE");
      let field = row
        .get("COLUMN_NAME")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

      match grouped.entry(name.clone()) {
        Entry::Occupied(mut entry) => {
          if !field.is_empty() {
            entry.get_mut().fields.push(field);
          }
        }
        Entry::Vacant(entry) => {
          order.push(name.clone());
          let fields = if field.is_empty() {
            Vec::new()
          } else {
            vec![field]
          };
          entry.insert(IndexInfo {
            name,
            collection: collection.to_string(),
            fields,
            index_type: "b-tree".to_string(),
            unique,
            sparse: false,
          });
        }
      }
    }

    Ok(
      order
        .into_iter()
        .filter_map(|name| grouped.remove(&name))
        .collect(),
    )
  }

}

#[async_trait]
impl SchemaIntrospection for OracleProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let sql = "SELECT table_name FROM user_tables ORDER BY table_name";
    let (_names, rows) = self.query_rows(sql, Vec::new()).await?;

    let mut collections = Vec::new();
    for row in rows {
      let name = row
        .get("TABLE_NAME")
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
    // `data_default` is a LONG column and is deliberately left out: fetching
    // it would fail on tables whose columns have defaults.
    let sql = "SELECT column_name, data_type, nullable FROM user_tab_columns WHERE table_name = ? ORDER BY column_id";
    let (_names, rows) = self
      .query_rows(sql, vec![Value::String(collection.to_string())])
      .await?;

    let mut fields = HashMap::new();
    for row in rows {
      let name = row
        .get("COLUMN_NAME")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
      if name.is_empty() {
        continue;
      }
      let field_type = row
        .get("DATA_TYPE")
        .and_then(Value::as_str)
        .unwrap_or("VARCHAR2")
        .to_string();
      let nullable = row.get("NULLABLE").and_then(Value::as_str) != Some("N");
      fields.insert(
        name.clone(),
        FieldInfo {
          name,
          field_type,
          nullable,
          default_value: None,
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
    // Segment bytes are the on-disk size of the table; a missing privilege
    // for `user_segments` degrades the size to zero, never the stats call.
    let sql = "SELECT SUM(bytes) AS size FROM user_segments WHERE segment_name = ? AND segment_type = 'TABLE'";
    let size = self
      .query_rows(sql, vec![Value::String(collection.to_string())])
      .await
      .ok()
      .and_then(|(_names, rows)| rows.first().cloned())
      .and_then(|row| row.get("SIZE").cloned())
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
    let sql = "SELECT SYS_CONTEXT('USERENV', 'DB_NAME') AS db_name FROM dual";
    let (_names, rows) = self.query_rows(sql, Vec::new()).await?;
    Ok(
      rows
        .first()
        .and_then(|row| {
          row
            .get("DB_NAME")
            .or_else(|| row.as_object().and_then(|map| map.values().next()))
        })
        .and_then(Value::as_str)
        .unwrap_or("oracle")
        .to_string(),
    )
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    // One Oracle connection is one database; the instance it reached is the
    // whole list rather than a guess at other installations.
    Ok(vec![self.get_database_name().await?])
  }
}

#[async_trait]
impl AdminCommands for OracleProvider {
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
    let sql = "SELECT banner FROM v$version WHERE ROWNUM = 1";
    let (_names, rows) = self.query_rows(sql, Vec::new()).await?;
    Ok(
      rows
        .first()
        .and_then(|row| {
          row
            .get("BANNER")
            .or_else(|| row.as_object().and_then(|map| map.values().next()))
        })
        .and_then(Value::as_str)
        .unwrap_or("oracle")
        .to_string(),
    )
  }

  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    let healthy = self.health_check().await?;
    let server_version = self.get_server_version().await?;
    Ok(ConnectionHealth {
      healthy,
      latency_ms: None,
      server_version: Some(server_version),
      connected_at: None,
      pool_stats: None,
    })
  }
}

// Oracle transactions are implicit: the session simply starts writing and
// something must commit. `begin` therefore marks the provider state instead
// of sending a statement, and `execute` stops auto-committing while it is
// active.
#[async_trait]
impl TransactionControl for OracleProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    let id = TransactionId::new(uuid::Uuid::new_v4().to_string());
    let mut guard = self.transaction_manager.lock().await;
    if guard.is_some() {
      return Err(OrmError::Transaction(
        "Transaction already active".to_string(),
      ));
    }
    *guard = Some(id.clone());
    Ok(id)
  }

  async fn commit_transaction(&self, id: TransactionId) -> OrmResult<()> {
    {
      let guard = self.transaction_manager.lock().await;
      match guard.as_ref() {
        Some(active_id) if active_id == &id => {}
        Some(_) => return Err(OrmError::Transaction("Transaction ID mismatch".to_string())),
        None => return Err(OrmError::Transaction("No active transaction".to_string())),
      }
    }

    self.with_connection(|conn| conn.commit()).await?;
    *self.transaction_manager.lock().await = None;
    Ok(())
  }

  async fn rollback_transaction(&self, id: TransactionId) -> OrmResult<()> {
    {
      let guard = self.transaction_manager.lock().await;
      match guard.as_ref() {
        Some(active_id) if active_id == &id => {}
        Some(_) => return Err(OrmError::Transaction("Transaction ID mismatch".to_string())),
        None => return Err(OrmError::Transaction("No active transaction".to_string())),
      }
    }

    self.with_connection(|conn| conn.rollback()).await?;
    *self.transaction_manager.lock().await = None;
    Ok(())
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(self.transaction_manager.lock().await.is_some())
  }
}

#[cfg(test)]
mod tests;
