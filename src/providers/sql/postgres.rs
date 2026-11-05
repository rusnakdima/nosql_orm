use crate::error::{map_err_connection, map_err_query, OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::provider::{
  AdminCommands, Capability, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  FieldInfo, IndexInfo, ProviderCapabilities, ProviderConfig, RawResult, SchemaIntrospection,
  Storage, TransactionControl, TransactionId,
};
use crate::providers::sql::common::SqlProviderHelpers;
use crate::providers::sql::row;
use crate::providers::sql::utils::{statement_kind, StatementKind};
use crate::query::{Filter, OrderBy, Projection};
use crate::sql::introspection::TableIntrospector;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use async_trait::async_trait;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone)]
pub struct PostgresProvider {
  pool: Pool,
  dialect: SqlDialect,
  query_builder: SqlQueryBuilder,
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
}

impl SqlProviderHelpers for PostgresProvider {
  fn dialect(&self) -> SqlDialect {
    self.dialect
  }

  fn query_builder(&self) -> &SqlQueryBuilder {
    &self.query_builder
  }
}

#[async_trait]
impl TableIntrospector for PostgresProvider {
  async fn table_columns(&self, table: &str) -> OrmResult<Vec<String>> {
    let sql = "SELECT a.attname::text
               FROM pg_catalog.pg_attribute a
               JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
               JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
               WHERE n.nspname = current_schema()
                 AND c.relname = $1
                 AND a.attnum > 0
                 AND NOT a.attisdropped
               ORDER BY a.attnum";

    let client = map_err_connection(self.pool.get().await)?;
    let rows = map_err_query(client.query(sql, &[&table]).await)?;

    let mut columns = Vec::with_capacity(rows.len());
    for row in rows {
      let name: String = row.get(0);
      columns.push(name);
    }
    Ok(columns)
  }
}

#[async_trait]
impl Storage for PostgresProvider {
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

    let sql = self.build_insert_sql(collection, &columns, &values);

    let client = map_err_connection(self.pool.get().await)?;

    let row = map_err_query(client.query_one(&sql, &[]).await)?;

    Ok(row::row_to_json_postgres(&row))
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let sql = self.build_select_by_id_sql(collection);

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
    let mut params: Vec<Value> = Vec::new();

    if let Some(f) = filter {
      let (clause, values) = self.query_builder.filter_to_sql_params(f)?;
      sql.push_str(" WHERE ");
      sql.push_str(&clause);
      params = values;
    }

    if let Some(sort) = sort_by {
      let dir = if sort_asc { "ASC" } else { "DESC" };
      sql.push_str(&format!(
        " ORDER BY {} {}",
        self.dialect.quote_identifier(sort),
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

    let client = map_err_connection(self.pool.get().await)?;

    let owned_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> =
      params.iter().map(to_postgres_param).collect();
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = owned_params
      .iter()
      .map(|param| param.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync))
      .collect();
    let rows = map_err_query(client.query(&sql, &param_refs).await)?;

    Ok(rows.iter().map(row::row_to_json_postgres).collect())
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let set_clauses = self.query_builder.build_set_clause(&doc, &["id"])?;

    let sql = self.build_update_by_id_sql(collection, &set_clauses);

    let client = map_err_connection(self.pool.get().await)?;

    let row = map_err_query(client.query_one(&sql, &[&id]).await)?;

    Ok(row::row_to_json_postgres(&row))
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let set_clauses = self.query_builder.build_set_clause(&patch, &[])?;

    let sql = self.build_update_by_id_sql(collection, &set_clauses);

    let client = map_err_connection(self.pool.get().await)?;

    let row = map_err_query(client.query_one(&sql, &[&id]).await)?;

    Ok(row::row_to_json_postgres(&row))
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let sql = self.build_delete_by_id_sql(collection);

    let client = map_err_connection(self.pool.get().await)?;

    let result = map_err_query(client.execute(&sql, &[&id]).await)?;

    Ok(result > 0)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let mut sql = self.build_count_sql(collection);
    let mut params: Vec<Value> = Vec::new();

    if let Some(f) = filter {
      let (clause, values) = self.query_builder.filter_to_sql_params(f)?;
      sql.push_str(" WHERE ");
      sql.push_str(&clause);
      params = values;
    }

    let client = map_err_connection(self.pool.get().await)?;

    let owned_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> =
      params.iter().map(to_postgres_param).collect();
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = owned_params
      .iter()
      .map(|param| param.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync))
      .collect();
    let row = map_err_query(client.query_one(&sql, &param_refs).await)?;

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

    let mut sql = self.build_update_sql(collection, &set_clauses);
    let mut params: Vec<Value> = Vec::new();

    if let Some(f) = filter {
      let (clause, values) = self.query_builder.filter_to_sql_params(&f)?;
      sql.push_str(" WHERE ");
      sql.push_str(&clause);
      params = values;
    }

    let client = map_err_connection(self.pool.get().await)?;

    let owned_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> =
      params.iter().map(to_postgres_param).collect();
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = owned_params
      .iter()
      .map(|param| param.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync))
      .collect();
    let rows = map_err_query(client.execute(&sql, &param_refs).await)?;

    Ok(rows as usize)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let mut sql = format!("DELETE FROM {}", self.dialect.quote_identifier(collection));
    let mut params: Vec<Value> = Vec::new();

    if let Some(f) = filter {
      let (clause, values) = self.query_builder.filter_to_sql_params(&f)?;
      sql.push_str(" WHERE ");
      sql.push_str(&clause);
      params = values;
    }

    let client = map_err_connection(self.pool.get().await)?;

    let owned_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> =
      params.iter().map(to_postgres_param).collect();
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = owned_params
      .iter()
      .map(|param| param.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync))
      .collect();
    let rows = map_err_query(client.execute(&sql, &param_refs).await)?;

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

  fn capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities::none()
      .with(Capability::Indexes)
      .with(Capability::RawQueries)
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

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    let sql = "SELECT datname FROM pg_database WHERE datistemplate = false";
    let client = map_err_connection(self.pool.get().await)?;
    let rows = map_err_query(client.query(sql, &[]).await)?;
    let names: Vec<String> = rows.iter().map(|row| row.get("datname")).collect();
    Ok(names)
  }
}

fn to_postgres_param(value: &Value) -> Box<dyn tokio_postgres::types::ToSql + Sync + Send> {
  match value {
    // NULL must bind as NULL; an empty string is a value, not the absence of one.
    Value::Null => Box::new(Option::<String>::None) as Box<dyn tokio_postgres::types::ToSql + Sync + Send>,
    Value::Bool(b) => Box::new(*b) as Box<dyn tokio_postgres::types::ToSql + Sync + Send>,
    Value::Number(n) => {
      if let Some(i) = n.as_i64() {
        Box::new(i) as Box<dyn tokio_postgres::types::ToSql + Sync + Send>
      } else if let Some(f) = n.as_f64() {
        Box::new(f) as Box<dyn tokio_postgres::types::ToSql + Sync + Send>
      } else {
        Box::new(n.to_string()) as Box<dyn tokio_postgres::types::ToSql + Sync + Send>
      }
    }
    Value::String(s) => Box::new(s.clone()) as Box<dyn tokio_postgres::types::ToSql + Sync + Send>,
    Value::Array(arr) => Box::new(serde_json::to_string(arr).unwrap_or_default())
      as Box<dyn tokio_postgres::types::ToSql + Sync + Send>,
    Value::Object(obj) => Box::new(serde_json::to_string(obj).unwrap_or_default())
      as Box<dyn tokio_postgres::types::ToSql + Sync + Send>,
  }
}

#[async_trait]
impl AdminCommands for PostgresProvider {
  async fn execute_raw(&self, query: &str, params: Vec<Value>) -> OrmResult<RawResult> {
    let client = map_err_connection(self.pool.get().await)?;

    // The boxed parameters own the values; the references borrow from them
    // for the duration of the call. Nothing is leaked.
    let owned_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> =
      params.iter().map(to_postgres_param).collect();
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = owned_params
      .iter()
      .map(|param| param.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync))
      .collect();

    if statement_kind(query) == StatementKind::Write {
      let affected_rows = map_err_query(client.execute(query, &param_refs).await)?;
      return Ok(RawResult {
        columns: vec![],
        rows: vec![],
        affected_rows,
        last_insert_id: None,
      });
    }

    let rows = map_err_query(client.query(query, &param_refs).await)?;

    let columns: Vec<String> = rows
      .first()
      .map(|row| row.columns().iter().map(|c| c.name().to_string()).collect())
      .unwrap_or_default();

    let rows_data: Vec<Vec<Value>> = rows
      .iter()
      .map(|row| {
        let json = row::row_to_json_postgres(row);
        columns
          .iter()
          .map(|name| json.get(name).cloned().unwrap_or(Value::Null))
          .collect()
      })
      .collect();

    Ok(RawResult {
      affected_rows: rows_data.len() as u64,
      columns,
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
    let client = map_err_connection(self.pool.get().await)?;
    map_err_query(client.execute(&sql, &[]).await)?;
    Ok(())
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    let sql = self.build_drop_table_sql(collection);
    let client = map_err_connection(self.pool.get().await)?;
    map_err_query(client.execute(&sql, &[]).await)?;
    Ok(())
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    let sql = self.build_rename_table_sql(from, to);
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

// Every operation takes a fresh connection from the pool, so BEGIN and COMMIT
// would land on different clients and no statement would actually be inside
// the transaction. A real implementation has to hand back a transaction-scoped
// provider; until then this is NotSupported rather than a false promise.
#[async_trait]
impl TransactionControl for PostgresProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    Err(OrmError::NotSupported(
      "the PostgreSQL provider uses a connection pool: a transaction needs one dedicated connection"
        .to_string(),
    ))
  }

  async fn commit_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the PostgreSQL provider cannot commit a transaction".to_string(),
    ))
  }

  async fn rollback_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the PostgreSQL provider cannot roll back a transaction".to_string(),
    ))
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(false)
  }
}
