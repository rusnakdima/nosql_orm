// DuckDB provider on the native `duckdb` crate.
//
// The layout mirrors the SQLite provider: one embedded `duckdb::Connection`
// behind a mutex for the provider's whole lifetime, every statement executed
// on a blocking task, and the shared `Storage` body coming from the
// `SqlExecutor` blanket impl. DuckDB is a synchronous C++ library, so the
// async surface is the same thin `spawn_blocking` shell SQLite uses.

use crate::error::{OrmError, OrmResult};
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  FieldInfo, IndexInfo, ProviderConfig, RawResult, SchemaIntrospection,
  Storage, TransactionControl, TransactionId,
};
use crate::providers::sql::common::SqlProviderHelpers;
use crate::providers::sql::utils::value_as_u64;
use crate::providers::sql::SqlExecutor;
use crate::query::{Filter, OrderBy, Projection};
use crate::sql::introspection::TableIntrospector;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct DuckDbProvider {
  conn: Arc<tokio::sync::Mutex<duckdb::Connection>>,
  dialect: SqlDialect,
  query_builder: SqlQueryBuilder,
  transaction_manager: Arc<tokio::sync::Mutex<Option<TransactionId>>>,
}

impl DuckDbProvider {
  pub async fn connect(path: impl AsRef<str>) -> OrmResult<Self> {
    let path_str = path.as_ref().to_string();
    let conn = tokio::task::spawn_blocking(move || {
      if path_str != ":memory:" {
        let db_path = Path::new(&path_str);
        if let Some(parent) = db_path.parent() {
          if !parent.exists() {
            std::fs::create_dir_all(parent)
              .map_err(|e| OrmError::Connection(format!("Failed to create directory: {}", e)))?;
          }
        }
      }

      duckdb::Connection::open(&path_str)
        .map_err(|e| OrmError::Connection(format!("Failed to open DuckDB: {}", e)))
    })
    .await
    .map_err(|e| OrmError::Connection(format!("Task join error: {}", e)))??;

    Ok(Self {
      conn: Arc::new(tokio::sync::Mutex::new(conn)),
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
    F: FnOnce(&duckdb::Connection) -> Result<R, duckdb::Error> + Send + 'static,
    R: Send + 'static,
  {
    let conn = self.conn.clone();
    let result = tokio::task::spawn_blocking(move || {
      let guard = conn.blocking_lock();
      f(&guard)
    })
    .await
    .map_err(|e| OrmError::Connection(format!("Task join error: {}", e)))?;
    result.map_err(|e| OrmError::Connection(format!("DuckDB error: {}", e)))
  }
}

// DuckDB's `ValueRef` is non-exhaustive and carries types (timestamps,
// decimals, lists) that have no JSON shape here; those are stringified so a
// read never drops a column silently.
fn duckdb_value_to_json(row: &duckdb::Row, index: usize) -> Value {
  use duckdb::types::ValueRef;

  match row.get_ref(index) {
    Err(_) => Value::Null,
    Ok(ValueRef::Null) => Value::Null,
    Ok(ValueRef::Boolean(b)) => serde_json::json!(b),
    Ok(ValueRef::TinyInt(v)) => serde_json::json!(v),
    Ok(ValueRef::SmallInt(v)) => serde_json::json!(v),
    Ok(ValueRef::Int(v)) => serde_json::json!(v),
    Ok(ValueRef::BigInt(v)) => serde_json::json!(v),
    Ok(ValueRef::UTinyInt(v)) => serde_json::json!(v),
    Ok(ValueRef::USmallInt(v)) => serde_json::json!(v),
    Ok(ValueRef::UInt(v)) => serde_json::json!(v),
    Ok(ValueRef::UBigInt(v)) => serde_json::json!(v),
    Ok(ValueRef::Float(v)) => serde_json::json!(v),
    Ok(ValueRef::Double(v)) => serde_json::json!(v),
    Ok(ValueRef::Text(bytes)) => Value::String(String::from_utf8_lossy(bytes).into_owned()),
    Ok(ValueRef::Blob(bytes)) => Value::String(crate::providers::sql::utils::base64_encode(bytes)),
    Ok(other) => Value::String(format!("{:?}", other)),
  }
}

fn duckdb_row_to_json(row: &duckdb::Row, names: &[String]) -> Value {
  let mut map = serde_json::Map::new();
  for (index, name) in names.iter().enumerate() {
    map.insert(name.clone(), duckdb_value_to_json(row, index));
  }
  Value::Object(map)
}

fn to_duckdb_param(value: &Value) -> duckdb::types::Value {
  match value {
    Value::Null => duckdb::types::Value::Null,
    Value::Bool(b) => duckdb::types::Value::Boolean(*b),
    Value::Number(n) => {
      if let Some(i) = n.as_i64() {
        duckdb::types::Value::BigInt(i)
      } else if let Some(u) = n.as_u64() {
        duckdb::types::Value::UBigInt(u)
      } else if let Some(f) = n.as_f64() {
        duckdb::types::Value::Double(f)
      } else {
        duckdb::types::Value::Text(n.to_string())
      }
    }
    Value::String(s) => duckdb::types::Value::Text(s.clone()),
    Value::Array(_) | Value::Object(_) => {
      duckdb::types::Value::Text(serde_json::to_string(value).unwrap_or_default())
    }
  }
}

impl SqlProviderHelpers for DuckDbProvider {
  fn dialect(&self) -> SqlDialect {
    self.dialect
  }

  fn query_builder(&self) -> &SqlQueryBuilder {
    &self.query_builder
  }
}

#[async_trait]
impl TableIntrospector for DuckDbProvider {
  async fn table_columns(&self, table: &str) -> OrmResult<Vec<String>> {
    let sql = "SELECT column_name FROM duckdb_columns() WHERE table_name = ? ORDER BY column_index";
    let rows = self
      .query(sql, vec![Value::String(table.to_string())])
      .await?;
    Ok(
      rows
        .iter()
        .filter_map(|row| {
          row
            .get("column_name")
            .and_then(Value::as_str)
            .map(str::to_string)
        })
        .collect(),
    )
  }
}

#[async_trait]
impl crate::providers::sql::SqlExecutor for DuckDbProvider {
  async fn execute(&self, sql: &str, params: Vec<Value>) -> OrmResult<u64> {
    let sql = sql.to_string();
    self
      .with_connection(move |conn_guard| {
        let rows = conn_guard.execute(
          &sql,
          duckdb::params_from_iter(params.iter().map(to_duckdb_param)),
        )?;
        Ok(rows as u64)
      })
      .await
  }

  async fn query(&self, sql: &str, params: Vec<Value>) -> OrmResult<Vec<Value>> {
    let sql = sql.to_string();
    self
      .with_connection(move |conn_guard| {
        let mut stmt = conn_guard.prepare(&sql)?;
        let mut rows = stmt.query(duckdb::params_from_iter(params.iter().map(to_duckdb_param)))?;
        // `column_names` panics on a statement that has not stepped yet, so
        // it is read through the row handle after `query` executed.
        let names: Vec<String> = rows
          .as_ref()
          .map(|statement| statement.column_names())
          .unwrap_or_default();
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
          results.push(duckdb_row_to_json(row, &names));
        }
        Ok(results)
      })
      .await
  }

  async fn list_indexes_for(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    // `SELECT *` keeps this working across DuckDB releases: the rows are
    // mapped by name below, so a renamed or added column is not an error.
    let sql = "SELECT * FROM duckdb_indexes() WHERE table_name = ?";
    let rows = self
      .query(sql, vec![Value::String(collection.to_string())])
      .await?;

    let indexes = rows
      .iter()
      .filter_map(|row| {
        let name = row.get("index_name")?.as_str()?.to_string();
        // A primary-key index belongs to the constraint: DROP INDEX cannot
        // remove it, so it is not reported as a droppable index.
        if row.get("is_primary").and_then(Value::as_bool) == Some(true) {
          return None;
        }
        let unique = row
          .get("is_unique")
          .and_then(Value::as_bool)
          .unwrap_or(false);
        // `duckdb_indexes()` reports the indexed expression as `[title]`;
        // the square brackets are the list, not the column names.
        let fields = row
          .get("expressions")
          .and_then(Value::as_str)
          .map(|expression| {
            expression
              .trim_matches(|c| c == '[' || c == ']')
              .split(',')
              .map(str::trim)
              .filter(|field| !field.is_empty())
              .map(str::to_string)
              .collect()
          })
          .unwrap_or_default();
        Some(IndexInfo {
          name,
          collection: collection.to_string(),
          fields,
          index_type: "art".to_string(),
          unique,
          sparse: false,
        })
      })
      .collect();

    Ok(indexes)
  }

}

#[async_trait]
impl SchemaIntrospection for DuckDbProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let sql = "SELECT table_name FROM duckdb_tables() WHERE internal = false AND schema_name = 'main' ORDER BY table_name";
    let rows = self.query(sql, Vec::new()).await?;

    let mut collections = Vec::new();
    for row in rows {
      let name = row
        .get("table_name")
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
    let sql = "SELECT column_name, data_type, is_nullable, column_default FROM duckdb_columns() WHERE table_name = ? ORDER BY column_index";
    let rows = self
      .query(sql, vec![Value::String(collection.to_string())])
      .await?;

    let mut fields = HashMap::new();
    for row in rows {
      let name = row
        .get("column_name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
      if name.is_empty() {
        continue;
      }
      let field_type = row
        .get("data_type")
        .and_then(Value::as_str)
        .unwrap_or("VARCHAR")
        .to_string();
      let nullable = row
        .get("is_nullable")
        .and_then(Value::as_bool)
        .unwrap_or(true);
      let default_value = row.get("column_default").cloned();
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
    // `estimated_size` is NULL for in-memory tables; a missing or failing
    // size query must not fail the stats call, so it degrades to zero.
    let size_sql = "SELECT estimated_size FROM duckdb_tables() WHERE table_name = ?";
    let size = self
      .query(size_sql, vec![Value::String(collection.to_string())])
      .await
      .ok()
      .and_then(|rows| {
        rows
          .first()
          .and_then(|row| row.get("estimated_size"))
          .and_then(value_as_u64)
      })
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
    let rows = self.query("SELECT current_database()", Vec::new()).await?;
    Ok(
      rows
        .first()
        .and_then(|row| row.get("current_database"))
        .and_then(Value::as_str)
        .unwrap_or("duckdb")
        .to_string(),
    )
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    let rows = self
      .query(
        "SELECT database_name FROM duckdb_databases() WHERE internal = false ORDER BY database_name",
        Vec::new(),
      )
      .await?;
    Ok(
      rows
        .iter()
        .filter_map(|row| {
          row
            .get("database_name")
            .and_then(Value::as_str)
            .map(str::to_string)
        })
        .collect(),
    )
  }
}

#[async_trait]
impl AdminCommands for DuckDbProvider {
  async fn execute_raw(&self, query: &str, params: Vec<Value>) -> OrmResult<RawResult> {
    let query = query.to_string();
    self
      .with_connection(move |conn_guard| {
        let mut stmt = conn_guard.prepare(&query)?;
        let column_count = stmt.column_count();

        if column_count == 0 {
          let affected_rows =
            stmt.execute(duckdb::params_from_iter(params.iter().map(to_duckdb_param)))?;
          return Ok(RawResult {
            columns: vec![],
            rows: vec![],
            affected_rows: affected_rows as u64,
            last_insert_id: None,
          });
        }

        let mut rows = stmt.query(duckdb::params_from_iter(params.iter().map(to_duckdb_param)))?;
        let columns: Vec<String> = rows
          .as_ref()
          .map(|statement| statement.column_names())
          .unwrap_or_default();
        let mut rows_data = Vec::new();
        while let Some(row) = rows.next()? {
          let values: Vec<Value> = columns
            .iter()
            .enumerate()
            .map(|(index, _)| duckdb_value_to_json(row, index))
            .collect();
          rows_data.push(values);
        }

        Ok(RawResult {
          affected_rows: rows_data.len() as u64,
          columns,
          rows: rows_data,
          last_insert_id: None,
        })
      })
      .await
  }

  async fn create_collection(
    &self,
    collection: &str,
    _schema: Option<CollectionSchema>,
  ) -> OrmResult<()> {
    let sql = self.build_create_table_sql(collection);
    self
      .with_connection(move |conn_guard| {
        conn_guard.execute_batch(&sql)?;
        Ok(())
      })
      .await
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    let sql = self.build_drop_table_sql(collection);
    self
      .with_connection(move |conn_guard| {
        conn_guard.execute_batch(&sql)?;
        Ok(())
      })
      .await
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    let sql = self.build_rename_table_sql(from, to);
    self
      .with_connection(move |conn_guard| {
        conn_guard.execute_batch(&sql)?;
        Ok(())
      })
      .await
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    let rows = self.query("SELECT version()", Vec::new()).await?;
    Ok(
      rows
        .first()
        .and_then(|row| row.get("version"))
        .and_then(Value::as_str)
        .unwrap_or("duckdb")
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

// DuckDB owns one connection for the provider's lifetime, so BEGIN, the
// statements in between and COMMIT/ROLLBACK all run on that connection —
// the same honour-the-trait shape as the SQLite provider.
#[async_trait]
impl TransactionControl for DuckDbProvider {
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

    let result = self
      .with_connection(move |conn_guard| {
        conn_guard.execute_batch("BEGIN TRANSACTION")?;
        Ok(())
      })
      .await;
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

    self
      .with_connection(move |conn_guard| {
        conn_guard.execute_batch("COMMIT")?;
        Ok(())
      })
      .await?;
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

    self
      .with_connection(move |conn_guard| {
        conn_guard.execute_batch("ROLLBACK")?;
        Ok(())
      })
      .await?;
    *self.transaction_manager.lock().await = None;
    Ok(())
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(self.transaction_manager.lock().await.is_some())
  }
}
