//! SQLite provider for nosql_orm.

use crate::error::{map_err_connection, OrmError, OrmResult};
use crate::nosql_index::{NosqlIndex, NosqlIndexInfo};
use crate::provider::{DatabaseProvider, ProviderConfig};
use crate::providers::sql::row;
use crate::query::Filter;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct SqliteProvider {
  conn: Arc<Mutex<rusqlite::Connection>>,
  dialect: SqlDialect,
  query_builder: SqlQueryBuilder,
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
      conn: Arc::new(Mutex::new(conn)),
      dialect: SqlDialect::SQLite,
      query_builder: SqlQueryBuilder::new(SqlDialect::SQLite),
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
    map_err_connection(
      tokio::task::spawn_blocking(move || {
        let conn_guard = conn.blocking_lock();
        f(&conn_guard)
      })
      .await
      .map_err(|e| OrmError::Connection(format!("Task join error: {}", e)))?,
    )
  }
}

#[async_trait]
impl DatabaseProvider for SqliteProvider {
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

    let placeholders: Vec<String> = (0..columns.len()).map(|_| "?".to_string()).collect();

    let sql = format!(
      "INSERT INTO {} ({}) VALUES ({})",
      self.dialect.quote_identifier(collection),
      columns
        .iter()
        .map(|c| self.dialect.quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", "),
      placeholders.join(", ")
    );

    let values: Vec<String> = columns
      .iter()
      .map(|c| {
        doc
          .get(*c)
          .map(|v| self.query_builder.value_to_sql(v))
          .unwrap_or_else(|| "NULL".to_string())
      })
      .collect();

    let _conn = self.conn.clone();
    let collection = collection.to_string();
    let id_clone = id.clone();
    let dialect = self.dialect;
    self
      .with_connection(move |conn_guard| {
        conn_guard.execute(&sql, rusqlite::params_from_iter(values.iter()))?;

        let sql = format!(
          "SELECT * FROM {} WHERE id = ?",
          dialect.quote_identifier(&collection)
        );
        let mut stmt = conn_guard.prepare(&sql)?;
        let result = stmt.query_row([id_clone], |row| row::row_to_json_sqlite(row));

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
        let result = stmt.query_row([id_owned], |row| row::row_to_json_sqlite(row));

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
      sql.push_str(&format!(" WHERE {}", self.query_builder.filter_to_sql(f)));
    }

    if let Some(sort) = sort_by {
      let dir = if sort_asc { "ASC" } else { "DESC" };
      sql.push_str(&format!(
        " ORDER BY {} {}",
        self.dialect.quote_identifier(sort),
        dir
      ));
    }

    if let Some(s) = skip {
      sql.push_str(&format!(" OFFSET {}", s));
    }

    if let Some(l) = limit {
      sql.push_str(&format!(" LIMIT {}", l));
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
        let result = stmt.query_row([id_owned.as_str()], |row| row::row_to_json_sqlite(row));

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
        let result = stmt.query_row([id_owned.as_str()], |row| row::row_to_json_sqlite(row));

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
      sql.push_str(&format!(" WHERE {}", self.query_builder.filter_to_sql(f)));
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
      sql.push_str(&format!(" WHERE {}", self.query_builder.filter_to_sql(&f)));
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
      sql.push_str(&format!(" WHERE {}", self.query_builder.filter_to_sql(&f)));
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

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<NosqlIndexInfo>> {
    Ok(vec![])
  }
}
