// SQL Server provider on the native `tiberius` TDS driver.
//
// One TCP connection is owned by the provider for its whole lifetime (like
// the SQLite and DuckDB providers), so BEGIN, the statements in between and
// COMMIT/ROLLBACK really share a connection and `TransactionControl` can be
// honoured instead of refused.
//
// T-SQL differs from the SQL the shared builders emit in three places, and
// `rewrite_portable_sql` fixes each one before a statement leaves this file: `?`
// placeholders become `@P1…`, the TRUE/FALSE literals T-SQL has no keyword
// for become 1/0, and `LIMIT n OFFSET m` becomes the OFFSET/FETCH pair the
// server understands.

use crate::error::{OrmError, OrmResult};
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  FieldInfo, IndexInfo, ProviderConfig, RawResult, SchemaIntrospection,
  Storage, TransactionControl, TransactionId,
};
use crate::providers::sql::common::SqlProviderHelpers;
use crate::providers::sql::utils::{
  replace_word, rewrite_portable_sql, statement_kind, value_as_u64, PlaceholderStyle, StatementKind,
};
use crate::providers::sql::SqlExecutor;
use crate::query::{Filter, OrderBy, Projection};
use crate::sql::introspection::TableIntrospector;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::compat::TokioAsyncWriteCompatExt;

type TdsStream = tokio_util::compat::Compat<tokio::net::TcpStream>;

#[derive(Clone)]
pub struct MssqlProvider {
  client: Arc<tokio::sync::Mutex<tiberius::Client<TdsStream>>>,
  dialect: SqlDialect,
  query_builder: SqlQueryBuilder,
  transaction_manager: Arc<tokio::sync::Mutex<Option<TransactionId>>>,
}

impl MssqlProvider {
  // The connection string is an ADO-style string, the shape tiberius parses:
  // `server=tcp:localhost,1433;user id=sa;password=…;TrustServerCertificate=true`.
  pub async fn connect(connection: impl AsRef<str>) -> OrmResult<Self> {
    let config = tiberius::Config::from_ado_string(connection.as_ref())
      .map_err(|e| OrmError::Connection(format!("Invalid SQL Server connection string: {}", e)))?;
    Self::connect_with_config(config).await
  }

  pub async fn connect_with_config(config: tiberius::Config) -> OrmResult<Self> {
    let tcp = tokio::net::TcpStream::connect(config.get_addr())
      .await
      .map_err(|e| OrmError::Connection(format!("SQL Server connect: {}", e)))?;
    tcp
      .set_nodelay(true)
      .map_err(|e| OrmError::Connection(format!("SQL Server set_nodelay: {}", e)))?;

    let mut client = tiberius::Client::connect(config, tcp.compat_write())
      .await
      .map_err(|e| OrmError::Connection(format!("SQL Server handshake: {}", e)))?;

    // The dialect quotes identifiers with double quotes, so the session must
    // parse them as identifiers no matter how the server is configured.
    client
      .query("SET QUOTED_IDENTIFIER ON", &[])
      .await
      .map_err(|e| OrmError::Connection(format!("SQL Server session setup: {}", e)))?
      .into_results()
      .await
      .map_err(|e| OrmError::Connection(format!("SQL Server session setup: {}", e)))?;

    Ok(Self {
      client: Arc::new(tokio::sync::Mutex::new(client)),
      // Double-quoted identifiers and `?` placeholders: quoting is valid
      // T-SQL (QUOTED_IDENTIFIER is on by default) and the placeholders are
      // rewritten below.
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

  async fn query_rows(
    &self,
    sql: &str,
    params: Vec<Value>,
  ) -> OrmResult<(Vec<String>, Vec<Value>)> {
    let sql = rewrite_portable_sql(sql, PlaceholderStyle::Tsql);
    let mut client = self.client.lock().await;
    let mut query = tiberius::Query::new(sql);
    for param in &params {
      bind_param(&mut query, param);
    }

    let stream = query
      .query(&mut client)
      .await
      .map_err(|e| OrmError::Query(format!("SQL Server query: {}", e)))?;
    let result_sets = stream
      .into_results()
      .await
      .map_err(|e| OrmError::Query(format!("SQL Server query: {}", e)))?;

    let mut names: Vec<String> = Vec::new();
    let mut rows: Vec<Value> = Vec::new();
    for result_set in result_sets {
      for row in result_set {
        if names.is_empty() {
          names = row
            .columns()
            .iter()
            .map(|column| column.name().to_string())
            .collect();
        }
        rows.push(tiberius_row_to_json(&row));
      }
    }
    Ok((names, rows))
  }
}

fn bind_param(query: &mut tiberius::Query<'_>, value: &Value) {
  match value {
    Value::Null => query.bind(Option::<&str>::None),
    Value::Bool(b) => query.bind(*b),
    Value::Number(n) => {
      if let Some(i) = n.as_i64() {
        query.bind(i);
      } else if let Some(u) = n.as_u64() {
        match i64::try_from(u) {
          Ok(i) => query.bind(i),
          Err(_) => query.bind(n.as_f64().unwrap_or(0.0)),
        }
      } else if let Some(f) = n.as_f64() {
        query.bind(f);
      } else {
        query.bind(n.to_string());
      }
    }
    Value::String(s) => query.bind(s.clone()),
    other => query.bind(serde_json::to_string(other).unwrap_or_default()),
  }
}

// T-SQL has no way to say "any type": `try_get` distinguishes a type
// mismatch (Err) from a NULL (Ok(None)), so the first successful read wins
// and an all-mismatch column stays NULL instead of being guessed.
fn tiberius_row_to_json(row: &tiberius::Row) -> Value {
  let mut map = serde_json::Map::new();
  for (index, column) in row.columns().iter().enumerate() {
    // `try_get` only takes borrowed `FromSql` types for text and bytes, so
    // those two are read by reference and converted here.
    let value = if let Ok(Some(v)) = row.try_get::<&str, _>(index) {
      serde_json::json!(v)
    } else if let Ok(Some(v)) = row.try_get::<i64, _>(index) {
      serde_json::json!(v)
    } else if let Ok(Some(v)) = row.try_get::<i32, _>(index) {
      serde_json::json!(v)
    } else if let Ok(Some(v)) = row.try_get::<f64, _>(index) {
      serde_json::json!(v)
    } else if let Ok(Some(v)) = row.try_get::<f32, _>(index) {
      serde_json::json!(v)
    } else if let Ok(Some(v)) = row.try_get::<bool, _>(index) {
      serde_json::json!(v)
    } else if let Ok(Some(v)) = row.try_get::<&[u8], _>(index) {
      Value::String(crate::providers::sql::utils::base64_encode(v))
    } else {
      Value::Null
    };
    map.insert(column.name().to_string(), value);
  }
  Value::Object(map)
}

// The shared DDL is `CREATE TABLE x (id TEXT PRIMARY KEY)`. SQL Server
// deprecates TEXT and forbids it as a key column, so the column types are
// mapped before the statement is sent.
fn rewrite_create_table_tsql(sql: &str) -> Option<String> {
  let rest = sql.trim_start().strip_prefix("CREATE TABLE ")?;
  let (if_not_exists, rest) = match rest.strip_prefix("IF NOT EXISTS ") {
    Some(tail) => ("IF NOT EXISTS ", tail),
    None => ("", rest),
  };
  let open = rest.find(" (")?;
  let close = rest.rfind(')')?;
  if close < open {
    return None;
  }

  let mut columns = rest[open + 2..close].to_string();
  let conversions = [
    ("TEXT", "NVARCHAR(450)"),
    ("BOOLEAN", "BIT"),
    ("TIMESTAMP", "DATETIME2"),
    ("DATETIME", "DATETIME2"),
    ("INTEGER", "INT"),
    ("DOUBLE", "FLOAT"),
    ("REAL", "FLOAT"),
    ("JSON", "NVARCHAR(MAX)"),
  ];
  for (from, to) in conversions {
    columns = replace_word(&columns, from, to);
  }

  let mut out = String::with_capacity(sql.len() + 16);
  out.push_str("CREATE TABLE ");
  out.push_str(if_not_exists);
  out.push_str(&rest[..open]);
  out.push_str(" (");
  out.push_str(columns.trim());
  out.push(')');
  Some(out)
}

impl SqlProviderHelpers for MssqlProvider {
  fn dialect(&self) -> SqlDialect {
    self.dialect
  }

  fn query_builder(&self) -> &SqlQueryBuilder {
    &self.query_builder
  }
}

#[async_trait]
impl TableIntrospector for MssqlProvider {
  async fn table_columns(&self, table: &str) -> OrmResult<Vec<String>> {
    let sql = "SELECT COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_NAME = ? ORDER BY ORDINAL_POSITION";
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
impl crate::providers::sql::SqlExecutor for MssqlProvider {
  async fn execute(&self, sql: &str, params: Vec<Value>) -> OrmResult<u64> {
    // The shared drop-index statement is the SQLite shape (`DROP INDEX x`,
    // no table); T-SQL needs `DROP INDEX x ON table` and the table is not in
    // the statement, so this is refused with a reason instead of sent.
    if is_tableless_drop_index(sql) {
      return Err(OrmError::NotSupported(
        "drop_index on SQL Server needs the table name, which the shared statement does not carry"
          .to_string(),
      ));
    }

    let sql = rewrite_portable_sql(sql, PlaceholderStyle::Tsql);
    let mut client = self.client.lock().await;
    let mut query = tiberius::Query::new(sql);
    for param in &params {
      bind_param(&mut query, param);
    }
    let result = query
      .execute(&mut client)
      .await
      .map_err(|e| OrmError::Query(format!("SQL Server write: {}", e)))?;
    Ok(result.total())
  }

  async fn query(&self, sql: &str, params: Vec<Value>) -> OrmResult<Vec<Value>> {
    let (_names, rows) = self.query_rows(sql, params).await?;
    Ok(rows)
  }

  async fn list_indexes_for(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    let sql = "SELECT i.name AS name, i.is_unique AS is_unique, c.name AS column_name
               FROM sys.indexes i
               JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id
               JOIN sys.columns c ON c.object_id = i.object_id AND ic.column_id = c.column_id
               WHERE i.object_id = OBJECT_ID(?) AND i.name IS NOT NULL AND i.is_primary_key = 0
               ORDER BY i.name, ic.key_ordinal";
    let (_names, rows) = self
      .query_rows(sql, vec![Value::String(collection.to_string())])
      .await?;

    let mut order: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, IndexInfo> = HashMap::new();
    for row in rows {
      let name = match row.get("name").and_then(Value::as_str) {
        Some(name) => name.to_string(),
        None => continue,
      };
      let unique = row
        .get("is_unique")
        .and_then(Value::as_bool)
        .unwrap_or(false);
      let field = row
        .get("column_name")
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

fn is_tableless_drop_index(sql: &str) -> bool {
  let trimmed = sql.trim_start();
  trimmed.len() >= 11
    && trimmed
      .get(..11)
      .map_or(false, |head| head.eq_ignore_ascii_case("DROP INDEX "))
    && !trimmed
      .split(|c: char| c.is_whitespace())
      .any(|word| word.eq_ignore_ascii_case("ON"))
}

#[async_trait]
impl SchemaIntrospection for MssqlProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let sql = "SELECT TABLE_NAME FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_TYPE = 'BASE TABLE' ORDER BY TABLE_NAME";
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
    let sql = "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_NAME = ? ORDER BY ORDINAL_POSITION";
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
        .unwrap_or("nvarchar")
        .to_string();
      let nullable = row
        .get("IS_NULLABLE")
        .and_then(Value::as_str)
        .map(|value| value.eq_ignore_ascii_case("YES"))
        .unwrap_or(true);
      let default_value = row
        .get("COLUMN_DEFAULT")
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
    // Reserved pages of the table's partitions, bytes at 8192 per page. The
    // dynamic view needs an elevated permission, so a denial degrades the
    // size to zero instead of failing the stats call.
    let sql = "SELECT COALESCE(SUM(reserved_page_count), 0) * 8192 AS size FROM sys.dm_db_partition_stats WHERE object_id = OBJECT_ID(?)";
    let size = self
      .query_rows(sql, vec![Value::String(collection.to_string())])
      .await
      .ok()
      .and_then(|(_names, rows)| rows.first().cloned())
      .and_then(|row| row.get("size").cloned())
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
      .query_rows("SELECT DB_NAME() AS name", Vec::new())
      .await?;
    Ok(
      rows
        .first()
        .and_then(|row| {
          row
            .get("name")
            .or_else(|| row.as_object().and_then(|map| map.values().next()))
        })
        .and_then(Value::as_str)
        .unwrap_or("mssql")
        .to_string(),
    )
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    let (_names, rows) = self
      .query_rows("SELECT name FROM sys.databases ORDER BY name", Vec::new())
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
impl AdminCommands for MssqlProvider {
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
    // The shared DDL carries SQL types T-SQL rejects on a key column (TEXT);
    // only this ORM-generated statement is translated — raw user DDL sent
    // through `execute_raw` is passed to the server as written.
    let sql = self.build_create_table_sql(collection);
    let sql = rewrite_create_table_tsql(&sql).unwrap_or(sql);
    self.execute(&sql, Vec::new()).await?;
    Ok(())
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    let sql = self.build_drop_table_sql(collection);
    self.execute(&sql, Vec::new()).await?;
    Ok(())
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    // T-SQL renames objects through a procedure; there is no
    // `ALTER TABLE … RENAME TO`.
    let sql = format!(
      "EXEC sp_rename '{}', '{}'",
      from.replace('\'', "''"),
      to.replace('\'', "''")
    );
    self.execute(&sql, Vec::new()).await?;
    Ok(())
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    let (_names, rows) = self
      .query_rows("SELECT @@VERSION AS version", Vec::new())
      .await?;
    Ok(
      rows
        .first()
        .and_then(|row| {
          row
            .get("version")
            .or_else(|| row.as_object().and_then(|map| map.values().next()))
        })
        .and_then(Value::as_str)
        .unwrap_or("mssql")
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

// The provider owns one connection, so BEGIN and COMMIT/ROLLBACK act on the
// same session every other statement uses — the same shape as SQLite and
// DuckDB, and unlike the pooled Postgres/MySQL providers.
#[async_trait]
impl TransactionControl for MssqlProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    let id = TransactionId::new(uuid::Uuid::new_v4().to_string());
    {
      let mut guard = self.transaction_manager.lock().await;
      if guard.is_some() {
        return Err(OrmError::Transaction(
          "Transaction already active".to_string(),
        ));
      }
      *guard = Some(id.clone());
    }

    let result = self.execute("BEGIN TRANSACTION", Vec::new()).await;
    if result.is_err() {
      *self.transaction_manager.lock().await = None;
    }
    result?;
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

    self.execute("COMMIT", Vec::new()).await?;
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

    self.execute("ROLLBACK", Vec::new()).await?;
    *self.transaction_manager.lock().await = None;
    Ok(())
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(self.transaction_manager.lock().await.is_some())
  }
}

#[cfg(test)]
mod tests;
