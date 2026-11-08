// The one SQL document-store implementation.
//
// `Storage` for the three SQL backends used to be three near-copies of the
// same bodies: identical document preparation, identical SELECT/UPDATE/DELETE
// building, identical filter/sort/limit assembly — differing only in how a
// connection is acquired and how rows come back. That duplication was the
// Phase 4 collapse (roadmap register A3): the CRUD core now lives here, once,
// behind [`SqlExecutor`], and each backend supplies only its execution
// adapter (`execute`, `query`, index listing, capabilities).

use crate::admin_types::IndexInfo;
use crate::error::{OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::provider::{ProviderCapabilities, Storage};
use crate::providers::sql::common::SqlProviderHelpers;
use crate::query::Filter;
use async_trait::async_trait;
use serde_json::Value;

// What a SQL backend must provide for the shared `Storage` implementation.
//
// SQL text is built by [`SqlProviderHelpers`] (dialect-aware); the adapter
// only runs it and maps rows to JSON.
#[async_trait]
pub trait SqlExecutor: SqlProviderHelpers + Send + Sync + Clone + 'static {
  // Run a statement that returns no rows; the value is the affected-row count.
  async fn execute(&self, sql: &str, params: Vec<Value>) -> OrmResult<u64>;

  // Run a statement that returns rows; every row becomes a JSON object.
  async fn query(&self, sql: &str, params: Vec<Value>) -> OrmResult<Vec<Value>>;

  // The backend's own index introspection (PRAGMA / SHOW / pg_indexes).
  async fn list_indexes_for(&self, collection: &str) -> OrmResult<Vec<IndexInfo>>;

  // The backend's capability set (e.g. only SQLite claims Transactions).
  fn sql_capabilities(&self) -> ProviderCapabilities;
}

#[async_trait]
impl<E: SqlExecutor> Storage for E {
  async fn insert(&self, collection: &str, mut doc: Value) -> OrmResult<Value> {
    let id = doc
      .get("id")
      .and_then(|value| value.as_str())
      .map(str::to_string)
      .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    doc["id"] = Value::String(id.clone());

    let columns: Vec<String> = doc
      .as_object()
      .map(|map| map.keys().cloned().collect())
      .unwrap_or_default();
    let values: Vec<String> = columns
      .iter()
      .map(|column| {
        doc
          .get(column)
          .map(|value| self.value_to_sql(value))
          .unwrap_or_else(|| "NULL".to_string())
      })
      .collect();
    let refs: Vec<&str> = columns.iter().map(String::as_str).collect();

    // A plain INSERT (no RETURNING) then a read-back: the read-back is the
    // one flow all three backends share, so the document that comes back is
    // assembled by the same code everywhere.
    let sql = self.build_insert_plain_sql(collection, &refs, &values);
    self.execute(&sql, Vec::new()).await?;

    self
      .find_by_id(collection, &id)
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("Inserted document not found: {}", id)))
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let sql = self.build_select_by_id_sql(collection);
    let params = vec![Value::String(id.to_string())];
    let mut rows = self.query(&sql, params).await?;
    Ok(rows.pop())
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
    let mut sql = self.build_select_all_sql(collection);
    let mut params: Vec<Value> = Vec::new();

    if let Some(f) = filter {
      let (clause, values) = self.query_builder().filter_to_sql_params(f)?;
      sql.push_str(" WHERE ");
      sql.push_str(&clause);
      params = values;
    }

    if let Some(sort) = sort_by {
      let dir = if sort_asc { "ASC" } else { "DESC" };
      sql.push_str(&format!(
        " ORDER BY {} {}",
        self.quote_identifier(sort),
        dir
      ));
    }

    // SQL reads "LIMIT n OFFSET m" — the other order is a syntax error on
    // SQLite and MySQL.
    if let Some(l) = limit {
      sql.push_str(&format!(" LIMIT {}", l));
    }

    if let Some(s) = skip {
      sql.push_str(&format!(" OFFSET {}", s));
    }

    self.query(&sql, params).await
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let set_clauses = self.query_builder().build_set_clause(&doc, &["id"])?;
    let sql = self.build_update_by_id_plain_sql(collection, &set_clauses);
    let params = vec![Value::String(id.to_string())];
    self.execute(&sql, params).await?;

    self
      .find_by_id(collection, id)
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("Document not found: {}", id)))
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let set_clauses = self.query_builder().build_set_clause(&patch, &[])?;
    let sql = self.build_update_by_id_plain_sql(collection, &set_clauses);
    let params = vec![Value::String(id.to_string())];
    self.execute(&sql, params).await?;

    self
      .find_by_id(collection, id)
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("Document not found: {}", id)))
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let sql = self.build_delete_by_id_sql(collection);
    let params = vec![Value::String(id.to_string())];
    Ok(self.execute(&sql, params).await? > 0)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let mut sql = self.build_delete_sql(collection);
    let mut params: Vec<Value> = Vec::new();

    if let Some(f) = filter {
      let (clause, values) = self.query_builder().filter_to_sql_params(&f)?;
      sql.push_str(" WHERE ");
      sql.push_str(&clause);
      params = values;
    }

    Ok(self.execute(&sql, params).await? as usize)
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    let set_clauses = self.query_builder().build_set_clause(&updates, &[])?;
    let mut sql = self.build_update_sql(collection, &set_clauses);
    let mut params: Vec<Value> = Vec::new();

    if let Some(f) = filter {
      let (clause, values) = self.query_builder().filter_to_sql_params(&f)?;
      sql.push_str(" WHERE ");
      sql.push_str(&clause);
      params = values;
    }

    Ok(self.execute(&sql, params).await? as usize)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let mut sql = self.build_count_sql(collection);
    let mut params: Vec<Value> = Vec::new();

    if let Some(f) = filter {
      let (clause, values) = self.query_builder().filter_to_sql_params(f)?;
      sql.push_str(" WHERE ");
      sql.push_str(&clause);
      params = values;
    }

    let rows = self.query(&sql, params).await?;
    let first = rows
      .first()
      .and_then(|row| row.as_object())
      .and_then(|map| map.values().next());
    match first {
      Some(Value::Number(n)) => Ok(
        n.as_u64()
          .or_else(|| n.as_i64().map(|v| v as u64))
          .unwrap_or(0),
      ),
      Some(Value::String(s)) => Ok(s.parse::<u64>().unwrap_or(0)),
      _ => Ok(0),
    }
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

    let sql = self.query_builder().build_create_index(&index_def);
    self.execute(&sql, Vec::new()).await?;
    Ok(())
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    let sql = self.build_drop_index_sql(collection, index_name);
    self.execute(&sql, Vec::new()).await?;
    Ok(())
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    self.list_indexes_for(collection).await
  }

  fn capabilities(&self) -> ProviderCapabilities {
    self.sql_capabilities()
  }
}
