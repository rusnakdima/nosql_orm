//! MySQL provider for nosql_orm.

use crate::error::{map_err_connection, map_err_query, OrmError, OrmResult};
use crate::nosql_index::{NosqlIndex, NosqlIndexInfo};
use crate::provider::{DatabaseProvider, ProviderConfig};
use crate::providers::sql::row;
use crate::query::Filter;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use async_trait::async_trait;
use mysql_async::prelude::*;
use mysql_async::{Opts, Pool, Row};
use serde_json::Value as JsonValue;

#[derive(Clone)]
pub struct MySqlProvider {
  pool: Pool,
  dialect: SqlDialect,
  query_builder: SqlQueryBuilder,
}

impl MySqlProvider {
  pub async fn connect(uri: impl AsRef<str>) -> OrmResult<Self> {
    let uri_str = uri.as_ref();

    let opts = Opts::from_url(uri_str)
      .map_err(|e| OrmError::Connection(format!("Invalid MySQL connection string: {}", e)))?;

    let pool = Pool::new(opts);

    Ok(Self {
      pool,
      dialect: SqlDialect::MySQL,
      query_builder: SqlQueryBuilder::new(SqlDialect::MySQL),
    })
  }

  pub async fn from_config(config: &ProviderConfig) -> OrmResult<Self> {
    Self::connect(&config.connection).await
  }

  pub fn dialect(&self) -> SqlDialect {
    self.dialect
  }
}

#[async_trait]
impl DatabaseProvider for MySqlProvider {
  async fn insert(&self, collection: &str, mut doc: JsonValue) -> OrmResult<JsonValue> {
    let id = if doc.get("id").is_none() {
      uuid::Uuid::new_v4().to_string()
    } else {
      doc["id"]
        .as_str()
        .map(String::from)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
    };
    doc["id"] = serde_json::json!(id);

    let columns: Vec<&str> = doc
      .as_object()
      .map(|m| m.keys().map(|k| k.as_str()).collect())
      .unwrap_or_default();

    let values: Vec<String> = columns
      .iter()
      .map(|c| {
        doc
          .get(*c)
          .map(|v| self.query_builder.value_to_sql(v))
          .unwrap_or_else(|| "NULL".to_string())
      })
      .collect();

    let sql = format!(
      "INSERT INTO {} ({}) VALUES ({})",
      self.dialect.quote_identifier(collection),
      columns
        .iter()
        .map(|c| self.dialect.quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", "),
      values.join(", ")
    );

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    map_err_query(conn.exec_drop(&sql, ()).await)?;

    self
      .find_by_id(collection, &id)
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("Inserted document not found: {}", id)))
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<JsonValue>> {
    let sql = format!(
      "SELECT * FROM {} WHERE id = ?",
      self.dialect.quote_identifier(collection)
    );

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    let result: Option<Row> = map_err_query(
      map_err_query(conn.exec_iter(&sql, (id,)).await)?
        .next()
        .await,
    )?;

    match result {
      Some(r) => Ok(Some(row::row_to_json_mysql(r))),
      None => Ok(None),
    }
  }

  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<&str>,
    sort_asc: bool,
  ) -> OrmResult<Vec<JsonValue>> {
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
      sql.push_str(&format!(" LIMIT {}, 18446744073709551615", s));
    }

    if let Some(l) = limit {
      sql.push_str(&format!(" LIMIT {}", l));
    }

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    let result: Vec<Row> = map_err_query(
      map_err_query(conn.exec_iter(&sql, ()).await)?
        .collect()
        .await,
    )?;

    let mut results = Vec::new();
    for row in result {
      results.push(row::row_to_json_mysql(row));
    }

    Ok(results)
  }

  async fn update(&self, collection: &str, id: &str, doc: JsonValue) -> OrmResult<JsonValue> {
    let set_clauses = self.query_builder.build_set_clause(&doc, &["id"])?;

    let sql = format!(
      "UPDATE {} SET {} WHERE id = ?",
      self.dialect.quote_identifier(collection),
      set_clauses.join(", ")
    );

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    map_err_query(conn.exec_drop(&sql, (id,)).await)?;

    self
      .find_by_id(collection, id)
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("Document not found: {}", id)))
  }

  async fn patch(&self, collection: &str, id: &str, patch: JsonValue) -> OrmResult<JsonValue> {
    let set_clauses = self.query_builder.build_set_clause(&patch, &[])?;

    let sql = format!(
      "UPDATE {} SET {} WHERE id = ?",
      self.dialect.quote_identifier(collection),
      set_clauses.join(", ")
    );

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    map_err_query(conn.exec_drop(&sql, (id,)).await)?;

    self
      .find_by_id(collection, id)
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("Document not found: {}", id)))
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let sql = format!(
      "DELETE FROM {} WHERE id = ?",
      self.dialect.quote_identifier(collection)
    );

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    map_err_query(conn.exec_drop(&sql, (id,)).await)?;

    Ok(true)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let mut sql = format!(
      "SELECT COUNT(*) FROM {}",
      self.dialect.quote_identifier(collection)
    );

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.query_builder.filter_to_sql(f)));
    }

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    let (count,): (i64,) = map_err_query(conn.exec_first(&sql, ()).await)?
      .ok_or_else(|| OrmError::Query("No result".to_string()))?;

    Ok(count as u64)
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: JsonValue,
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

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    map_err_query(conn.exec_drop(&sql, ()).await)?;

    Ok(conn.affected_rows() as usize)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let mut sql = format!("DELETE FROM {}", self.dialect.quote_identifier(collection));

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.query_builder.filter_to_sql(&f)));
    }

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    map_err_query(conn.exec_drop(&sql, ()).await)?;

    Ok(conn.affected_rows() as usize)
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

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    map_err_query(conn.exec_drop(&sql, ()).await)?;

    Ok(())
  }

  async fn drop_index(&self, _collection: &str, index_name: &str) -> OrmResult<()> {
    let sql = format!(
      "DROP INDEX {} ON {}",
      self.dialect.quote_identifier(index_name),
      self.dialect.quote_identifier(_collection)
    );

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    map_err_query(conn.exec_drop(&sql, ()).await)?;

    Ok(())
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<NosqlIndexInfo>> {
    Ok(vec![])
  }
}
