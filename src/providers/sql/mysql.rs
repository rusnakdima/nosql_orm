//! MySQL provider for nosql_orm.

use crate::error::{map_err_connection, map_err_query, OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  DatabaseProvider, FieldInfo, IndexInfo, ProviderConfig, RawResult, SchemaIntrospection,
  TransactionControl, TransactionId,
};
use crate::providers::sql::row;
use crate::query::Filter;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use async_trait::async_trait;
use mysql_async::prelude::*;
use mysql_async::{Opts, Pool, Row};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MySqlProvider {
  pool: Pool,
  dialect: SqlDialect,
  query_builder: SqlQueryBuilder,
  transaction_manager: Arc<Mutex<Option<TransactionId>>>,
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
      transaction_manager: Arc::new(Mutex::new(None)),
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

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Ok(vec![])
  }
}

#[async_trait]
impl SchemaIntrospection for MySqlProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let sql = "SHOW TABLES";

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let rows: Vec<Row> = map_err_query(
      map_err_query(conn.exec_iter(sql, ()).await)?
        .collect()
        .await,
    )?;

    let mut collections = Vec::new();
    for row in rows {
      let json = row::row_to_json_mysql(row);
      let name = json.get("Tables_in_database").or_else(|| json.get(0));
      match name {
        Some(name_val) => {
          let name_str = name_val.as_str().unwrap_or("").to_string();
          let count_sql = format!(
            "SELECT COUNT(*) FROM {}",
            self.dialect.quote_identifier(&name_str)
          );
          let (count,): (i64,) =
            map_err_query(conn.exec_first(&count_sql, ()).await)?.unwrap_or((0,));
          collections.push(CollectionMeta {
            name: name_str,
            document_count: count as u64,
            size_bytes: 0,
            created_at: None,
            updated_at: None,
          });
        }
        None => continue,
      }
    }
    Ok(collections)
  }

  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema> {
    let sql = format!("DESCRIBE {}", self.dialect.quote_identifier(collection));

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let rows: Vec<Row> = map_err_query(
      map_err_query(conn.exec_iter(&sql, ()).await)?
        .collect()
        .await,
    )?;

    let mut fields = HashMap::new();
    for row in rows {
      let json = row::row_to_json_mysql(row.clone());
      let name = json
        .get("Field")
        .or_else(|| json.get("Field"))
        .ok_or_else(|| OrmError::Query("Missing Field column".to_string()))?
        .as_str()
        .ok_or_else(|| OrmError::Query("Field value is not a string".to_string()))?
        .to_string();
      let field_type = json
        .get("Type")
        .or_else(|| json.get("Type"))
        .ok_or_else(|| OrmError::Query("Missing Type column".to_string()))?
        .as_str()
        .ok_or_else(|| OrmError::Query("Type value is not a string".to_string()))?
        .to_string();
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
  }

  async fn get_collection_stats(&self, collection: &str) -> OrmResult<CollectionStats> {
    let count_sql = format!(
      "SELECT COUNT(*) FROM {}",
      self.dialect.quote_identifier(collection)
    );
    let size_sql =
      "SELECT (data_length + index_length) FROM information_schema.tables WHERE table_name = ?";

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let (count,): (i64,) = map_err_query(conn.exec_first(&count_sql, ()).await)?.unwrap_or((0,));

    let size: i64 = match conn.exec_iter(&size_sql, (&collection,)).await {
      Ok(mut r) => match r.collect::<Row>().await {
        Ok(rows) => rows
          .first()
          .and_then(|row| row.get::<i64, _>(0))
          .unwrap_or(0),
        Err(_) => 0,
      },
      Err(_) => 0,
    };

    Ok(CollectionStats {
      name: collection.to_string(),
      document_count: count as u64,
      size_bytes: size as u64,
      storage_size_bytes: size as u64,
      index_count: 0,
      index_size_bytes: 0,
      average_document_size: if count > 0 { size / count } else { 0 } as u64,
    })
  }

  async fn list_indexes(&self, _collection: &str) -> OrmResult<Vec<IndexInfo>> {
    Ok(vec![])
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    let sql = "SELECT DATABASE()";
    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let (name,): (String,) =
      map_err_query(conn.exec_first(sql, ()).await)?.unwrap_or(("unknown".to_string(),));
    Ok(name)
  }
}

#[async_trait]
impl AdminCommands for MySqlProvider {
  async fn execute_raw(&self, query: &str, _params: Vec<JsonValue>) -> OrmResult<RawResult> {
    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    let is_select = query.trim().to_uppercase().starts_with("SELECT");

    if is_select {
      let rows: Vec<Row> = map_err_query(
        map_err_query(conn.exec_iter(query, ()).await)?
          .collect()
          .await,
      )?;

      if rows.is_empty() {
        return Ok(RawResult {
          columns: vec![],
          rows: vec![],
          affected_rows: 0,
          last_insert_id: None,
        });
      }

      let columns: Vec<String> = rows[0]
        .columns()
        .iter()
        .map(|c| c.name_str().as_ref().to_string())
        .collect();

      let rows_data: Vec<Vec<JsonValue>> = rows
        .iter()
        .map(|row| {
          columns
            .iter()
            .enumerate()
            .map(|(i, _)| {
              row::row_to_json_mysql(row.clone())
                .get(i)
                .cloned()
                .unwrap_or(JsonValue::Null)
            })
            .collect()
        })
        .collect();

      Ok(RawResult {
        columns,
        rows: rows_data,
        affected_rows: 0,
        last_insert_id: None,
      })
    } else {
      map_err_query(conn.exec_drop(query, ()).await)?;
      Ok(RawResult {
        columns: vec![],
        rows: vec![],
        affected_rows: 0,
        last_insert_id: None,
      })
    }
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
    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    map_err_query(conn.exec_drop(&sql, ()).await)?;
    Ok(())
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    let sql = format!(
      "DROP TABLE IF EXISTS {}",
      self.dialect.quote_identifier(collection)
    );
    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    map_err_query(conn.exec_drop(&sql, ()).await)?;
    Ok(())
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    let sql = format!(
      "RENAME TABLE {} TO {}",
      self.dialect.quote_identifier(from),
      self.dialect.quote_identifier(to)
    );
    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    map_err_query(conn.exec_drop(&sql, ()).await)?;
    Ok(())
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    let sql = "SELECT VERSION()";
    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let (version,): (String,) =
      map_err_query(conn.exec_first(sql, ()).await)?.unwrap_or(("unknown".to_string(),));
    Ok(version)
  }

  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    let healthy = self.health_check().await.unwrap_or(false);
    let server_version = self
      .get_server_version()
      .await
      .unwrap_or_else(|_| "unknown".to_string());
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
impl TransactionControl for MySqlProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    let id = {
      let mut guard = self.transaction_manager.lock().unwrap();
      if guard.is_some() {
        return Err(OrmError::Transaction(
          "Transaction already active".to_string(),
        ));
      }
      let id = TransactionId::new(uuid::Uuid::new_v4().to_string());
      *guard = Some(id.clone());
      id
    };

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    map_err_query(conn.exec_drop("START TRANSACTION", ()).await)?;
    Ok(id)
  }

  async fn commit_transaction(&self, id: TransactionId) -> OrmResult<()> {
    {
      let mut guard = self.transaction_manager.lock().unwrap();
      match guard.as_ref() {
        Some(active_id) if active_id == &id => {
          *guard = None;
        }
        Some(_) => return Err(OrmError::Transaction("Transaction ID mismatch".to_string())),
        None => return Err(OrmError::Transaction("No active transaction".to_string())),
      }
    }

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    map_err_query(conn.exec_drop("COMMIT", ()).await)?;
    Ok(())
  }

  async fn rollback_transaction(&self, id: TransactionId) -> OrmResult<()> {
    {
      let mut guard = self.transaction_manager.lock().unwrap();
      match guard.as_ref() {
        Some(active_id) if active_id == &id => {
          *guard = None;
        }
        Some(_) => return Err(OrmError::Transaction("Transaction ID mismatch".to_string())),
        None => return Err(OrmError::Transaction("No active transaction".to_string())),
      }
    }

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    map_err_query(conn.exec_drop("ROLLBACK", ()).await)?;
    Ok(())
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(self.transaction_manager.lock().unwrap().is_some())
  }
}
