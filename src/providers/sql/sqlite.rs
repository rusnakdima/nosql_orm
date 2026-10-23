//! SQLite provider for nosql_orm.

use crate::error::{OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth, FieldInfo,
  IndexInfo, ProviderConfig, RawResult, SchemaIntrospection, Storage, TransactionControl,
  TransactionId,
};
use crate::providers::sql::common::build_insert_sql;
use crate::providers::sql::row;
use crate::query::Filter;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct SqliteProvider {
  conn: Arc<tokio::sync::Mutex<rusqlite::Connection>>,
  dialect: SqlDialect,
  query_builder: SqlQueryBuilder,
  transaction_manager: Arc<tokio::sync::Mutex<Option<TransactionId>>>,
}

impl SqliteProvider {
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

      rusqlite::Connection::open(&path_str)
        .map_err(|e| OrmError::Connection(format!("Failed to open SQLite: {}", e)))
    })
    .await
    .map_err(|e| OrmError::Connection(format!("Task join error: {}", e)))??;

    conn
      .execute_batch("PRAGMA foreign_keys = ON;")
      .map_err(|e| OrmError::Connection(format!("Failed to set pragma: {}", e)))?;

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

  async fn with_connection<R, F>(&self, f: F) -> OrmResult<R>
  where
    F: FnOnce(&rusqlite::Connection) -> Result<R, rusqlite::Error> + Send + 'static,
    R: Send + 'static,
  {
    let conn = self.conn.clone();
    let result = tokio::task::spawn_blocking(move || {
      let guard = conn.blocking_lock();
      f(&guard)
    })
    .await
    .map_err(|e| OrmError::Connection(format!("Task join error: {}", e)))?;
    result.map_err(|e| OrmError::Connection(format!("SQLite error: {}", e)))
  }
}

#[async_trait]
impl Storage for SqliteProvider {
  async fn insert(&self, collection: &str, mut doc: Value) -> OrmResult<Value> {
    let id = match doc["id"].as_str() {
      Some(s) => s.to_string(),
      None => uuid::Uuid::new_v4().to_string(),
    };
    doc["id"] = serde_json::json!(id);

    let columns: Vec<&str> = doc
      .as_object()
      .map(|m| m.keys().map(|k| k.as_str()).collect())
      .unwrap_or_default();

    fn json_to_sql_param(v: &Value) -> String {
      match v {
        Value::Null => "NULL".to_string(),
        Value::Bool(b) => {
          if *b {
            "TRUE".to_string()
          } else {
            "FALSE".to_string()
          }
        }
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        Value::Array(arr) => {
          let items = arr
            .iter()
            .map(json_to_sql_param)
            .collect::<Vec<_>>()
            .join(", ");
          format!("({})", items)
        }
        Value::Object(obj) => {
          let pairs = obj
            .iter()
            .map(|(k, v)| format!("{}: {}", k, json_to_sql_param(v)))
            .collect::<Vec<_>>()
            .join(", ");
          format!("'{{{}}}'", pairs)
        }
      }
    }

    let values: Vec<String> = columns
      .iter()
      .map(|c| {
        doc
          .get(*c)
          .map(json_to_sql_param)
          .unwrap_or_else(|| "NULL".to_string())
      })
      .collect();

    let sql_with_values = build_insert_sql(&self.dialect, collection, &columns, &values);

    let _conn = self.conn.clone();
    let collection = collection.to_string();
    let id_clone = id.clone();
    let dialect = self.dialect;
    self
      .with_connection(move |conn_guard| {
        conn_guard.execute(&sql_with_values, [])?;

        let sql = format!(
          "SELECT * FROM {} WHERE id = ?",
          dialect.quote_identifier(&collection)
        );
        let mut stmt = conn_guard.prepare(&sql)?;
        let result = stmt.query_row([id_clone], row::row_to_json_sqlite);

        match result {
          Ok(value) => Ok(value),
          Err(rusqlite::Error::QueryReturnedNoRows) => Err(rusqlite::Error::QueryReturnedNoRows),
          Err(e) => Err(e),
        }
      })
      .await
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let sql = format!(
      "SELECT * FROM {} WHERE id = ?",
      self.dialect.quote_identifier(collection)
    );

    let id_owned = id.to_string();
    self
      .with_connection(move |conn_guard| {
        let mut stmt = conn_guard.prepare(&sql)?;
        let result = stmt.query_row([id_owned], row::row_to_json_sqlite);

        match result {
          Ok(value) => Ok(Some(value)),
          Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
          Err(e) => Err(e),
        }
      })
      .await
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
    let mut sql = format!(
      "SELECT * FROM {}",
      self.dialect.quote_identifier(collection)
    );

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.query_builder.filter_to_sql(f)?));
    }

    if let Some(sort) = sort_by {
      let dir = if sort_asc { "ASC" } else { "DESC" };
      sql.push_str(&format!(
        " ORDER BY {} {}",
        self.dialect.quote_identifier(sort),
        dir
      ));
    }

    // SQL reads "LIMIT n OFFSET m" — the other order is a syntax error.
    if let Some(l) = limit {
      sql.push_str(&format!(" LIMIT {}", l));
    }

    if let Some(s) = skip {
      sql.push_str(&format!(" OFFSET {}", s));
    }

    self
      .with_connection(move |conn_guard| {
        let mut stmt = conn_guard.prepare(&sql)?;
        let mut results = Vec::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
          if let Ok(value) = row::row_to_json_sqlite(row) {
            results.push(value);
          }
        }
        Ok(results)
      })
      .await
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let set_clauses = self.query_builder.build_set_clause(&doc, &["id"])?;

    let sql = format!(
      "UPDATE {} SET {} WHERE id = ?",
      self.dialect.quote_identifier(collection),
      set_clauses.join(", ")
    );

    let id_owned = id.to_string();
    let collection = collection.to_string();
    let dialect = self.dialect;
    self
      .with_connection(move |conn_guard| {
        conn_guard.execute(&sql, [id_owned.as_str()])?;

        let sql = format!(
          "SELECT * FROM {} WHERE id = ?",
          dialect.quote_identifier(&collection)
        );
        let mut stmt = conn_guard.prepare(&sql)?;
        let result = stmt.query_row([id_owned.as_str()], row::row_to_json_sqlite);

        match result {
          Ok(value) => Ok(value),
          Err(rusqlite::Error::QueryReturnedNoRows) => Err(rusqlite::Error::QueryReturnedNoRows),
          Err(e) => Err(e),
        }
      })
      .await
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let set_clauses = self.query_builder.build_set_clause(&patch, &[])?;

    let sql = format!(
      "UPDATE {} SET {} WHERE id = ?",
      self.dialect.quote_identifier(collection),
      set_clauses.join(", ")
    );

    let id_owned = id.to_string();
    let collection = collection.to_string();
    let dialect = self.dialect;
    self
      .with_connection(move |conn_guard| {
        conn_guard.execute(&sql, [id_owned.as_str()])?;

        let sql = format!(
          "SELECT * FROM {} WHERE id = ?",
          dialect.quote_identifier(&collection)
        );
        let mut stmt = conn_guard.prepare(&sql)?;
        let result = stmt.query_row([id_owned.as_str()], row::row_to_json_sqlite);

        match result {
          Ok(value) => Ok(value),
          Err(rusqlite::Error::QueryReturnedNoRows) => Err(rusqlite::Error::QueryReturnedNoRows),
          Err(e) => Err(e),
        }
      })
      .await
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let sql = format!(
      "DELETE FROM {} WHERE id = ?",
      self.dialect.quote_identifier(collection)
    );

    let id_owned = id.to_string();
    self
      .with_connection(move |conn_guard| {
        let rows = conn_guard.execute(&sql, [id_owned.as_str()])?;
        Ok(rows > 0)
      })
      .await
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let mut sql = format!(
      "SELECT COUNT(*) FROM {}",
      self.dialect.quote_identifier(collection)
    );

    if let Some(f) = filter {
      let clause = self.query_builder.filter_to_sql(f)?;
      sql.push_str(" WHERE ");
      sql.push_str(clause.as_str());
    }

    self
      .with_connection(move |conn_guard| {
        let count: i64 = conn_guard.query_row(&sql, [], |row| row.get(0))?;
        Ok(count as u64)
      })
      .await
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    let set_clauses = self.query_builder.build_set_clause(&updates, &[])?;

    let mut sql = format!(
      "UPDATE {} SET {}",
      self.dialect.quote_identifier(collection),
      set_clauses.join(", ")
    );

    if let Some(f) = filter {
      let clause = self.query_builder.filter_to_sql(&f)?;
      sql.push_str(" WHERE ");
      sql.push_str(clause.as_str());
    }

    self
      .with_connection(move |conn_guard| {
        let rows = conn_guard.execute(&sql, [])?;
        Ok(rows)
      })
      .await
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let mut sql = format!("DELETE FROM {}", self.dialect.quote_identifier(collection));

    if let Some(f) = filter {
      let clause = self.query_builder.filter_to_sql(&f)?;
      sql.push_str(" WHERE ");
      sql.push_str(clause.as_str());
    }

    self
      .with_connection(move |conn_guard| {
        let rows = conn_guard.execute(&sql, [])?;
        Ok(rows)
      })
      .await
  }

  async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()> {
    let mut index_def = crate::sql::types::SqlIndexDef::new(
      index.get_name().unwrap_or("idx_default"),
      collection,
      index.get_fields().iter().map(|(f, _)| f.clone()).collect(),
    );

    if index.is_unique() {
      index_def = index_def.unique();
    }

    let sql = self.query_builder.build_create_index(&index_def);

    self
      .with_connection(move |conn_guard| {
        conn_guard.execute(&sql, [])?;
        Ok(())
      })
      .await
  }

  async fn drop_index(&self, _collection: &str, index_name: &str) -> OrmResult<()> {
    let sql = format!(
      "DROP INDEX IF EXISTS {}",
      self.dialect.quote_identifier(index_name)
    );

    self
      .with_connection(move |conn_guard| {
        conn_guard.execute(&sql, [])?;
        Ok(())
      })
      .await
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Ok(vec![])
  }
}

#[async_trait]
impl SchemaIntrospection for SqliteProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    self
      .with_connection(move |conn_guard| {
        let mut stmt = conn_guard.prepare(
          "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )?;
        let mut rows = stmt.query([])?;
        let mut collections = Vec::new();
        while let Some(row) = rows.next()? {
          let name: String = row.get(0)?;
          let count: i64 =
            conn_guard.query_row(&format!("SELECT COUNT(*) FROM {}", name), [], |r| r.get(0))?;
          collections.push(CollectionMeta {
            name,
            document_count: count as u64,
            size_bytes: 0,
            created_at: None,
            updated_at: None,
          });
        }
        Ok(collections)
      })
      .await
  }

  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema> {
    let dialect = self.dialect;
    let collection = collection.to_string();
    self
      .with_connection(move |conn_guard| {
        let mut stmt = conn_guard.prepare(&format!(
          "PRAGMA table_info({})",
          dialect.quote_identifier(&collection)
        ))?;
        let mut rows = stmt.query([])?;
        let mut fields = HashMap::new();
        while let Some(row) = rows.next()? {
          let name: String = row.get(1)?;
          let field_type: String = row.get(2)?;
          fields.insert(
            name.clone(),
            FieldInfo {
              name,
              field_type,
              nullable: true,
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
      })
      .await
  }

  async fn get_collection_stats(&self, collection: &str) -> OrmResult<CollectionStats> {
    let collection = collection.to_string();
    self
      .with_connection(move |conn_guard| {
        let count: i64 =
          conn_guard.query_row(&format!("SELECT COUNT(*) FROM {}", collection), [], |r| {
            r.get(0)
          })?;
        let page_count: i64 = conn_guard
          .query_row(
            &format!(
              "SELECT page_count FROM sqlite_master WHERE name = '{}'",
              collection
            ),
            [],
            |r| r.get(0),
          )
          .unwrap_or(0);
        let page_size: i64 = conn_guard
          .query_row("PRAGMA page_size", [], |r| r.get(0))
          .unwrap_or(4096);
        Ok(CollectionStats {
          name: collection.to_string(),
          document_count: count as u64,
          size_bytes: (page_count * page_size) as u64,
          storage_size_bytes: (page_count * page_size) as u64,
          index_count: 0,
          index_size_bytes: 0,
          average_document_size: if count > 0 {
            (page_count * page_size) / count
          } else {
            0
          } as u64,
        })
      })
      .await
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Ok(vec![])
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    Ok("sqlite".to_string())
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    Ok(vec!["sqlite".to_string()])
  }
}

#[async_trait]
impl AdminCommands for SqliteProvider {
  async fn execute_raw(&self, query: &str, _params: Vec<Value>) -> OrmResult<RawResult> {
    let query = query.to_string();
    self
      .with_connection(move |conn_guard| {
        let mut stmt = conn_guard.prepare(&query)?;
        let column_count = stmt.column_count();
        let columns: Vec<String> = (0..column_count)
          .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
          .collect();

        let mut rows = stmt.query([])?;
        let mut rows_data = Vec::new();
        while let Some(row) = rows.next()? {
          let mut row_values = Vec::new();
          for i in 0..column_count {
            let value: rusqlite::types::Value = row.get(i)?;
            let json_value = match value {
              rusqlite::types::Value::Null => Value::Null,
              rusqlite::types::Value::Integer(i) => Value::Number(i.into()),
              rusqlite::types::Value::Real(f) => Value::Number(
                serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0)),
              ),
              rusqlite::types::Value::Text(s) => Value::String(s),
              rusqlite::types::Value::Blob(b) => Value::String(format!("<blob {} bytes>", b.len())),
            };
            row_values.push(json_value);
          }
          rows_data.push(row_values);
        }
        // A statement that writes reports what it changed; a statement that
        // reads reports nothing — `changes` still holds the count of the
        // last write, which is not this statement's.
        let affected_rows = if columns.is_empty() {
          conn_guard.changes()
        } else {
          0
        };

        Ok(RawResult {
          columns,
          rows: rows_data,
          affected_rows,
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
    let sql = format!(
      "CREATE TABLE {} (id TEXT PRIMARY KEY)",
      self.dialect.quote_identifier(collection)
    );
    self
      .with_connection(move |conn_guard| {
        conn_guard.execute(&sql, [])?;
        Ok(())
      })
      .await
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    let sql = format!(
      "DROP TABLE IF EXISTS {}",
      self.dialect.quote_identifier(collection)
    );
    self
      .with_connection(move |conn_guard| {
        conn_guard.execute(&sql, [])?;
        Ok(())
      })
      .await
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    let sql = format!(
      "ALTER TABLE {} RENAME TO {}",
      self.dialect.quote_identifier(from),
      self.dialect.quote_identifier(to)
    );
    self
      .with_connection(move |conn_guard| {
        conn_guard.execute(&sql, [])?;
        Ok(())
      })
      .await
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    self
      .with_connection(move |conn_guard| {
        let version: String = conn_guard.query_row("SELECT sqlite_version()", [], |r| r.get(0))?;
        Ok(version)
      })
      .await
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

#[async_trait]
impl TransactionControl for SqliteProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    let mut guard = self.transaction_manager.lock().await;
    if guard.is_some() {
      return Err(OrmError::Transaction(
        "Transaction already active".to_string(),
      ));
    }
    let id = TransactionId::new(uuid::Uuid::new_v4().to_string());
    *guard = Some(id.clone());
    drop(guard);

    self
      .with_connection(move |conn_guard| {
        conn_guard.execute("BEGIN", [])?;
        Ok(())
      })
      .await?;
    Ok(id)
  }

  async fn commit_transaction(&self, id: TransactionId) -> OrmResult<()> {
    let mut guard = self.transaction_manager.lock().await;
    match guard.as_ref() {
      Some(active_id) if active_id == &id => {
        *guard = None;
        drop(guard);

        self
          .with_connection(move |conn_guard| {
            conn_guard.execute("COMMIT", [])?;
            Ok(())
          })
          .await
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
        drop(guard);

        self
          .with_connection(move |conn_guard| {
            conn_guard.execute("ROLLBACK", [])?;
            Ok(())
          })
          .await
      }
      Some(_) => Err(OrmError::Transaction("Transaction ID mismatch".to_string())),
      None => Err(OrmError::Transaction("No active transaction".to_string())),
    }
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(self.transaction_manager.lock().await.is_some())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::tempdir;

  /// Create the table a test writes to.
  ///
  /// The provider writes to the tables the database already has — it does
  /// not create them. A test that inserts must therefore make its table
  /// first, exactly as a user of the library would.
  async fn make_table(provider: &SqliteProvider, columns: &[(&str, &str)]) -> OrmResult<()> {
    let columns = columns
      .iter()
      .map(|(name, kind)| format!("{} {}", name, kind))
      .collect::<Vec<_>>()
      .join(", ");
    let sql = format!("CREATE TABLE users ({})", columns);
    provider.execute_raw(&sql, Vec::new()).await?;
    Ok(())
  }

  #[tokio::test]
  async fn sqlite_insert_find_by_id() -> OrmResult<()> {
    let temp = tempdir()?;
    let db_path = temp.path().join("test.db");
    let provider = SqliteProvider::connect(db_path.to_str().unwrap()).await?;
    make_table(
      &provider,
      &[("id", "TEXT"), ("name", "TEXT"), ("email", "TEXT")],
    )
    .await?;

    let doc = serde_json::json!({
      "id": "test-id-1",
      "name": "Alice",
      "email": "alice@example.com"
    });
    let inserted = provider.insert("users", doc.clone()).await?;
    assert_eq!(inserted["id"], "test-id-1");
    assert_eq!(inserted["name"], "Alice");

    let found = provider.find_by_id("users", "test-id-1").await?;
    assert!(found.is_some());
    assert_eq!(found.unwrap()["name"], "Alice");

    Ok(())
  }

  #[tokio::test]
  async fn sqlite_insert_delete() -> OrmResult<()> {
    let temp = tempdir()?;
    let db_path = temp.path().join("test.db");
    let provider = SqliteProvider::connect(db_path.to_str().unwrap()).await?;
    make_table(&provider, &[("id", "TEXT"), ("name", "TEXT")]).await?;

    let doc = serde_json::json!({"id": "delete-me", "name": "Bob"});
    provider.insert("users", doc).await?;

    let found_before = provider.find_by_id("users", "delete-me").await?;
    assert!(found_before.is_some());

    provider.delete("users", "delete-me").await?;

    let found_after = provider.find_by_id("users", "delete-me").await?;
    assert!(found_after.is_none());

    Ok(())
  }

  #[tokio::test]
  async fn sqlite_update() -> OrmResult<()> {
    let temp = tempdir()?;
    let db_path = temp.path().join("test.db");
    let provider = SqliteProvider::connect(db_path.to_str().unwrap()).await?;
    make_table(
      &provider,
      &[("id", "TEXT"), ("name", "TEXT"), ("age", "INTEGER")],
    )
    .await?;

    let doc = serde_json::json!({"id": "update-me", "name": "Charlie", "age": 30});
    provider.insert("users", doc).await?;

    let updated = provider
      .update(
        "users",
        "update-me",
        serde_json::json!({"name": "Charles", "age": 31}),
      )
      .await?;
    assert_eq!(updated["name"], "Charles");
    assert_eq!(updated["age"], 31);

    let found = provider.find_by_id("users", "update-me").await?;
    assert!(found.is_some());
    assert_eq!(found.unwrap()["name"], "Charles");

    Ok(())
  }

  #[tokio::test]
  async fn sqlite_find_many_pages_and_sorts() -> OrmResult<()> {
    let temp = tempdir()?;
    let db_path = temp.path().join("test.db");
    let provider = SqliteProvider::connect(db_path.to_str().unwrap()).await?;
    make_table(&provider, &[("id", "TEXT"), ("name", "TEXT")]).await?;

    for (id, name) in [("a", "Ann"), ("b", "Bob"), ("c", "Cid")] {
      provider
        .insert("users", serde_json::json!({"id": id, "name": name}))
        .await?;
    }

    let all = provider
      .find_many("users", None, Some(0), Some(10), None, true)
      .await?;
    assert_eq!(all.len(), 3);

    let page = provider
      .find_many("users", None, Some(0), Some(2), None, true)
      .await?;
    assert_eq!(page.len(), 2);

    let sorted = provider
      .find_many("users", None, Some(0), Some(10), Some("name"), false)
      .await?;
    assert_eq!(sorted[0]["name"], "Cid");

    let counted = provider.count("users", None).await?;
    assert_eq!(counted, 3);

    Ok(())
  }

  #[tokio::test]
  async fn sqlite_list_collections_counts_the_rows() -> OrmResult<()> {
    let temp = tempdir()?;
    let db_path = temp.path().join("test.db");
    let provider = SqliteProvider::connect(db_path.to_str().unwrap()).await?;
    make_table(&provider, &[("id", "TEXT"), ("name", "TEXT")]).await?;
    provider
      .insert("users", serde_json::json!({"id": "u1", "name": "Ann"}))
      .await?;

    let collections = provider.list_collections().await?;
    let users = collections
      .iter()
      .find(|c| c.name == "users")
      .expect("the users table is listed");
    assert_eq!(users.document_count, 1);

    Ok(())
  }
}
