//! PostgreSQL provider for nosql_orm.

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
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// PostgreSQL-backed provider.
#[derive(Clone)]
pub struct PostgresProvider {
  pool: Pool,
  dialect: SqlDialect,
  query_builder: SqlQueryBuilder,
  transaction_manager: Arc<tokio::sync::Mutex<Option<TransactionId>>>,
}

impl PostgresProvider {
  pub async fn connect(uri: impl AsRef<str>) -> OrmResult<Self> {
    let uri_str = uri.as_ref();

    let pg_config: tokio_postgres::Config = uri_str
      .parse()
      .map_err(|e| OrmError::Connection(format!("Invalid PostgreSQL connection string: {}", e)))?;

    let mgr_config = ManagerConfig {
      recycling_method: RecyclingMethod::Fast,
    };
    let mgr = Manager::from_config(pg_config, tokio_postgres::NoTls, mgr_config);
    let pool = Pool::builder(mgr)
      .max_size(16)
      .build()
      .map_err(|e| OrmError::Connection(format!("Failed to create pool: {}", e)))?;

    Ok(Self {
      pool,
      dialect: SqlDialect::PostgreSQL,
      query_builder: SqlQueryBuilder::new(SqlDialect::PostgreSQL),
      transaction_manager: Arc::new(tokio::sync::Mutex::new(None)),
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
impl DatabaseProvider for PostgresProvider {
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
      "INSERT INTO {} ({}) VALUES ({}) RETURNING *",
      self.dialect.quote_identifier(collection),
      columns
        .iter()
        .map(|c| self.dialect.quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", "),
      values.join(", ")
    );

    let client = map_err_connection(self.pool.get().await)?;

    let row = map_err_query(client.query_one(&sql, &[]).await)?;

    Ok(row::row_to_json_postgres(&row))
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let sql = format!(
      "SELECT * FROM {} WHERE id = $1",
      self.dialect.quote_identifier(collection)
    );

    let client = map_err_connection(self.pool.get().await)?;

    let row = map_err_query(client.query_opt(&sql, &[&id]).await)?;

    Ok(row.map(|r| row::row_to_json_postgres(&r)))
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

    let _params: Vec<String> = Vec::new();
    let _param_idx = 0;

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

    if let Some(s) = skip {
      sql.push_str(&format!(" OFFSET {}", s));
    }

    if let Some(l) = limit {
      sql.push_str(&format!(" LIMIT {}", l));
    }

    let client = map_err_connection(self.pool.get().await)?;

    let rows = map_err_query(client.query(&sql, &[]).await)?;

    Ok(rows.iter().map(row::row_to_json_postgres).collect())
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let set_clauses = self.query_builder.build_set_clause(&doc, &["id"])?;

    let sql = format!(
      "UPDATE {} SET {} WHERE id = $1 RETURNING *",
      self.dialect.quote_identifier(collection),
      set_clauses.join(", ")
    );

    let client = map_err_connection(self.pool.get().await)?;

    let row = map_err_query(client.query_one(&sql, &[&id]).await)?;

    Ok(row::row_to_json_postgres(&row))
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let set_clauses = self.query_builder.build_set_clause(&patch, &[])?;

    let sql = format!(
      "UPDATE {} SET {} WHERE id = $1 RETURNING *",
      self.dialect.quote_identifier(collection),
      set_clauses.join(", ")
    );

    let client = map_err_connection(self.pool.get().await)?;

    let row = map_err_query(client.query_one(&sql, &[&id]).await)?;

    Ok(row::row_to_json_postgres(&row))
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let sql = format!(
      "DELETE FROM {} WHERE id = $1",
      self.dialect.quote_identifier(collection)
    );

    let client = map_err_connection(self.pool.get().await)?;

    let result = map_err_query(client.execute(&sql, &[&id]).await)?;

    Ok(result > 0)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let mut sql = format!(
      "SELECT COUNT(*) FROM {}",
      self.dialect.quote_identifier(collection)
    );

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.query_builder.filter_to_sql(f)?));
    }

    let client = map_err_connection(self.pool.get().await)?;

    let row = map_err_query(client.query_one(&sql, &[]).await)?;

    let count: i64 = row.get(0);
    Ok(count as u64)
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

    let client = map_err_connection(self.pool.get().await)?;

    let rows = map_err_query(client.execute(&sql, &[]).await)?;

    Ok(rows as usize)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let mut sql = format!("DELETE FROM {}", self.dialect.quote_identifier(collection));

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.query_builder.filter_to_sql(&f)));
    }

    let client = map_err_connection(self.pool.get().await)?;

    let rows = map_err_query(client.execute(&sql, &[]).await)?;

    Ok(rows as usize)
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

    let client = map_err_connection(self.pool.get().await)?;

    map_err_query(client.execute(&sql, &[]).await)?;

    Ok(())
  }

  async fn drop_index(&self, _collection: &str, index_name: &str) -> OrmResult<()> {
    let sql = format!(
      "DROP INDEX IF EXISTS {}",
      self.dialect.quote_identifier(index_name)
    );

    let client = map_err_connection(self.pool.get().await)?;

    map_err_query(client.execute(&sql, &[]).await)?;

    Ok(())
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    let sql = "
            SELECT indexname, indexdef
            FROM pg_indexes
            WHERE schemaname = 'public' AND tablename = $1
        ";

    let client = map_err_connection(self.pool.get().await)?;
    let rows = map_err_query(client.query(sql, &[&collection]).await)?;

    let indexes = rows
      .iter()
      .map(|row| {
        let name: String = row.get("indexname");
        let indexdef: String = row.get("indexdef");
        let unique = indexdef.contains("UNIQUE");
        let fields = extract_index_fields(&indexdef);

        IndexInfo {
          name,
          collection: collection.to_string(),
          fields: fields.into_iter().map(|(f, _)| f).collect(),
          index_type: determine_index_type(&indexdef),
          unique,
          sparse: false,
        }
      })
      .collect();

    Ok(indexes)
  }
}

fn extract_index_fields(indexdef: &str) -> Vec<(String, i32)> {
  let fields_re = regex::Regex::new(r"\((\w+)(?:\s+ASC|\s+DESC)?\)").unwrap();
  fields_re
    .captures_iter(indexdef)
    .map(|c| (c.get(1).unwrap().as_str().to_string(), 1i32))
    .collect()
}

fn determine_index_type(indexdef: &str) -> String {
  if indexdef.contains(" USING gin") {
    "gin".to_string()
  } else if indexdef.contains(" USING gist") {
    "gist".to_string()
  } else if indexdef.contains(" USING hash") {
    "hash".to_string()
  } else {
    "b-tree".to_string()
  }
}

#[async_trait]
impl SchemaIntrospection for PostgresProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let sql = "
            SELECT table_name
            FROM information_schema.tables
            WHERE table_schema = 'public'
        ";

    let client = map_err_connection(self.pool.get().await)?;
    let rows = map_err_query(client.query(sql, &[]).await)?;

    let mut collections = Vec::new();
    for row in rows {
      let name: String = row.get("table_name");
      let count_sql = format!(
        "SELECT COUNT(*) FROM {}",
        self.dialect.quote_identifier(&name)
      );
      let count_row = map_err_query(client.query_one(&count_sql, &[]).await)?;
      let count: i64 = count_row.get(0);
      collections.push(CollectionMeta {
        name,
        document_count: count as u64,
        size_bytes: 0,
        created_at: None,
        updated_at: None,
      });
    }
    Ok(collections)
  }

  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema> {
    let sql = "
            SELECT column_name, data_type
            FROM information_schema.columns
            WHERE table_schema = 'public' AND table_name = $1
        ";

    let client = map_err_connection(self.pool.get().await)?;
    let rows = map_err_query(client.query(sql, &[&collection]).await)?;

    let mut fields = HashMap::new();
    for row in rows {
      let name: String = row.get("column_name");
      let data_type: String = row.get("data_type");
      fields.insert(
        name.clone(),
        FieldInfo {
          name,
          field_type: data_type,
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
    let size_sql = format!(
      "SELECT pg_total_relation_size('{}')",
      self.dialect.quote_identifier(collection)
    );

    let client = map_err_connection(self.pool.get().await)?;
    let count_row = map_err_query(client.query_one(&count_sql, &[]).await)?;
    let count: i64 = count_row.get(0);

    let size_row = map_err_query(client.query_one(&size_sql, &[]).await)?;
    let size: i64 = size_row.get(0);

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

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    let sql = "
            SELECT indexname, indexdef
            FROM pg_indexes
            WHERE schemaname = 'public' AND tablename = $1
        ";

    let client = map_err_connection(self.pool.get().await)?;
    let rows = map_err_query(client.query(sql, &[&collection]).await)?;

    let indexes = rows
      .iter()
      .map(|row| {
        let name: String = row.get("indexname");
        let indexdef: String = row.get("indexdef");
        let unique = indexdef.contains("UNIQUE");
        let fields = extract_index_fields(&indexdef);

        IndexInfo {
          name,
          collection: collection.to_string(),
          fields: fields.into_iter().map(|(f, _)| f).collect(),
          index_type: determine_index_type(&indexdef),
          unique,
          sparse: false,
        }
      })
      .collect();
    Ok(indexes)
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    let sql = "SELECT current_database()";
    let client = map_err_connection(self.pool.get().await)?;
    let row = map_err_query(client.query_one(sql, &[]).await)?;
    let name: String = row.get(0);
    Ok(name)
  }
}

fn to_postgres_param(value: &Value) -> Box<dyn tokio_postgres::types::ToSql + Sync> {
  match value {
    Value::Null => Box::new(String::new()) as Box<dyn tokio_postgres::types::ToSql + Sync>,
    Value::Bool(b) => Box::new(*b) as Box<dyn tokio_postgres::types::ToSql + Sync>,
    Value::Number(n) => {
      if let Some(i) = n.as_i64() {
        Box::new(i) as Box<dyn tokio_postgres::types::ToSql + Sync>
      } else if let Some(f) = n.as_f64() {
        Box::new(f) as Box<dyn tokio_postgres::types::ToSql + Sync>
      } else {
        Box::new(n.to_string()) as Box<dyn tokio_postgres::types::ToSql + Sync>
      }
    }
    Value::String(s) => Box::new(s.clone()) as Box<dyn tokio_postgres::types::ToSql + Sync>,
    Value::Array(arr) => Box::new(serde_json::to_string(arr).unwrap_or_default())
      as Box<dyn tokio_postgres::types::ToSql + Sync>,
    Value::Object(obj) => Box::new(serde_json::to_string(obj).unwrap_or_default())
      as Box<dyn tokio_postgres::types::ToSql + Sync>,
  }
}

#[async_trait]
impl AdminCommands for PostgresProvider {
  async fn execute_raw(&self, query: &str, params: Vec<Value>) -> OrmResult<RawResult> {
    let client = map_err_connection(self.pool.get().await)?;

    let params_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
      params.iter().map(|v| Self::to_postgres_param(v)).collect();

    let rows = map_err_query(client.query(query, &params_refs).await)?;

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
      .map(|c| c.name().to_string())
      .collect();

    let rows_data: Vec<Vec<Value>> = rows
      .iter()
      .map(|row| {
        columns
          .iter()
          .enumerate()
          .map(|(i, _)| {
            row::row_to_json_postgres(row)
              .get(i)
              .cloned()
              .unwrap_or(Value::Null)
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
    let client = map_err_connection(self.pool.get().await)?;
    map_err_query(client.execute(&sql, &[]).await)?;
    Ok(())
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    let sql = format!(
      "DROP TABLE IF EXISTS {}",
      self.dialect.quote_identifier(collection)
    );
    let client = map_err_connection(self.pool.get().await)?;
    map_err_query(client.execute(&sql, &[]).await)?;
    Ok(())
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    let sql = format!(
      "ALTER TABLE {} RENAME TO {}",
      self.dialect.quote_identifier(from),
      self.dialect.quote_identifier(to)
    );
    let client = map_err_connection(self.pool.get().await)?;
    map_err_query(client.execute(&sql, &[]).await)?;
    Ok(())
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    let sql = "SELECT version()";
    let client = map_err_connection(self.pool.get().await)?;
    let row = map_err_query(client.query_one(sql, &[]).await)?;
    let version: String = row.get(0);
    Ok(version)
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
impl TransactionControl for PostgresProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    let id = {
      let mut guard = self.transaction_manager.lock().await;
      if guard.is_some() {
        return Err(OrmError::Transaction(
          "Transaction already active".to_string(),
        ));
      }
      let id = TransactionId::new(uuid::Uuid::new_v4().to_string());
      *guard = Some(id.clone());
      id
    };

    let client = map_err_connection(self.pool.get().await)?;
    map_err_query(client.execute("BEGIN", &[]).await)?;
    Ok(id)
  }

  async fn commit_transaction(&self, id: TransactionId) -> OrmResult<()> {
    {
      let mut guard = self.transaction_manager.lock().await;
      match guard.as_ref() {
        Some(active_id) if active_id == &id => {
          *guard = None;
        }
        Some(_) => return Err(OrmError::Transaction("Transaction ID mismatch".to_string())),
        None => return Err(OrmError::Transaction("No active transaction".to_string())),
      }
    }

    let client = map_err_connection(self.pool.get().await)?;
    map_err_query(client.execute("COMMIT", &[]).await)?;
    Ok(())
  }

  async fn rollback_transaction(&self, id: TransactionId) -> OrmResult<()> {
    {
      let mut guard = self.transaction_manager.lock().await;
      match guard.as_ref() {
        Some(active_id) if active_id == &id => {
          *guard = None;
        }
        Some(_) => return Err(OrmError::Transaction("Transaction ID mismatch".to_string())),
        None => return Err(OrmError::Transaction("No active transaction".to_string())),
      }
    }

    let client = map_err_connection(self.pool.get().await)?;
    map_err_query(client.execute("ROLLBACK", &[]).await)?;
    Ok(())
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(self.transaction_manager.lock().await.is_some())
  }
}
