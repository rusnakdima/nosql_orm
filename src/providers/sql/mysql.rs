use crate::error::{map_err_connection, map_err_query, OrmError, OrmResult};
use crate::nosql_index::NosqlIndex;
use crate::provider::{
  AdminCommands, Capability, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  FieldInfo, IndexInfo, ProviderCapabilities, ProviderConfig, RawResult, SchemaIntrospection,
  Storage, TransactionControl, TransactionId,
};
use crate::providers::sql::common::SqlProviderHelpers;
use crate::providers::sql::row;
use crate::query::{Filter, OrderBy, Projection};
use crate::sql::introspection::TableIntrospector;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use async_trait::async_trait;
use mysql_async::prelude::*;
use mysql_async::{Opts, Pool, Row};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, HashMap};

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

  pub async fn list_mysql_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    let sql = format!(
      "SHOW INDEX FROM {}",
      self.dialect.quote_identifier(collection)
    );

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let rows: Vec<Row> = map_err_query(
      map_err_query(conn.exec_iter(&sql, ()).await)?
        .collect()
        .await,
    )?;

    // SHOW INDEX returns one row per indexed column; group them by Key_name
    // so a compound index is one IndexInfo with all of its fields.
    let mut indexes: BTreeMap<String, IndexInfo> = BTreeMap::new();
    for row in rows {
      let json = row::row_to_json_mysql(row);
      let name = json
        .get("Key_name")
        .or_else(|| json.get("key_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
      if name.is_empty() {
        continue;
      }
      let column = json
        .get("Column_name")
        .or_else(|| json.get("column_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
      let non_unique = json.get("Non_unique").or_else(|| json.get("non_unique"));
      let index_type = json
        .get("Index_type")
        .or_else(|| json.get("index_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("btree")
        .to_lowercase();

      let entry = indexes.entry(name.clone()).or_insert_with(|| IndexInfo {
        name,
        collection: collection.to_string(),
        fields: Vec::new(),
        index_type,
        unique: is_zero(non_unique),
        sparse: false,
      });
      if !column.is_empty() {
        entry.fields.push(column);
      }
    }

    Ok(indexes.into_values().collect())
  }

  pub async fn health_check(&self) -> OrmResult<bool> {
    let mut conn = self.pool.get_conn().await.map_err(OrmError::connection)?;
    conn.query_iter("SELECT 1").await.map_err(OrmError::query)?;
    Ok(true)
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

  pub fn dialect(&self) -> SqlDialect {
    self.dialect
  }
}

impl SqlProviderHelpers for MySqlProvider {
  fn dialect(&self) -> SqlDialect {
    self.dialect
  }

  fn query_builder(&self) -> &SqlQueryBuilder {
    &self.query_builder
  }
}

#[async_trait]
impl TableIntrospector for MySqlProvider {
  async fn table_columns(&self, table: &str) -> OrmResult<Vec<String>> {
    let sql = "SELECT COLUMN_NAME
               FROM INFORMATION_SCHEMA.COLUMNS
               WHERE TABLE_SCHEMA = DATABASE()
                 AND TABLE_NAME = ?
               ORDER BY ORDINAL_POSITION";

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let rows: Vec<Row> = map_err_query(
      map_err_query(conn.exec_iter(sql, (table,)).await)?
        .collect()
        .await,
    )?;

    let mut columns = Vec::with_capacity(rows.len());
    for row in rows {
      if let Some(name) = row.get::<String, _>(0) {
        columns.push(name);
      }
    }
    Ok(columns)
  }
}

// SHOW INDEX reports Non_unique as 0 or 1; row_to_json_mysql may hand it back
// as a number, boolean or string depending on the driver conversion.
fn is_zero(value: Option<&JsonValue>) -> bool {
  match value {
    Some(JsonValue::Number(n)) => n.as_i64() == Some(0),
    Some(JsonValue::Bool(b)) => !*b,
    Some(JsonValue::String(s)) => s == "0",
    _ => false,
  }
}

fn convert_params(params: Vec<JsonValue>) -> Vec<mysql_async::Value> {
  params
    .into_iter()
    .map(|v| match v {
      JsonValue::Null => mysql_async::Value::NULL,
      JsonValue::Bool(b) => mysql_async::Value::Int(if b { 1 } else { 0 }),
      JsonValue::Number(n) => {
        if let Some(i) = n.as_i64() {
          mysql_async::Value::Int(i)
        } else if let Some(f) = n.as_f64() {
          mysql_async::Value::Float(f as f32)
        } else {
          mysql_async::Value::NULL
        }
      }
      JsonValue::String(s) => mysql_async::Value::Bytes(s.into_bytes()),
      JsonValue::Array(arr) => {
        mysql_async::Value::Bytes(serde_json::to_string(&arr).unwrap_or_default().into_bytes())
      }
      JsonValue::Object(obj) => {
        mysql_async::Value::Bytes(serde_json::to_string(&obj).unwrap_or_default().into_bytes())
      }
    })
    .collect()
}

#[async_trait]
impl Storage for MySqlProvider {
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

    let sql = self.build_insert_sql(collection, &columns, &values);

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    map_err_query(conn.exec_drop(&sql, ()).await)?;

    self
      .find_by_id(collection, &id)
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("Inserted document not found: {}", id)))
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<JsonValue>> {
    let sql = self.build_select_by_id_sql(collection);

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

    let mut params: Vec<JsonValue> = Vec::new();

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

    // SQL reads "LIMIT n OFFSET m" — two LIMIT clauses are a syntax error.
    if let Some(l) = limit {
      sql.push_str(&format!(" LIMIT {}", l));
    }

    if let Some(s) = skip {
      sql.push_str(&format!(" OFFSET {}", s));
    }

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let values = convert_params(params);

    let mut result = if values.is_empty() {
      map_err_query(conn.exec_iter(&sql, ()).await)?
    } else {
      map_err_query(conn.exec_iter(&sql, &values).await)?
    };
    let rows: Vec<Row> = map_err_query(result.collect().await)?;
    let mut results = Vec::new();
    for row in rows {
      results.push(row::row_to_json_mysql(row));
    }

    Ok(results)
  }

  async fn update(&self, collection: &str, id: &str, doc: JsonValue) -> OrmResult<JsonValue> {
    let set_clauses = self.query_builder.build_set_clause(&doc, &["id"])?;

    let sql = self.build_update_by_id_sql(collection, &set_clauses);

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
    let sql = self.build_delete_by_id_sql(collection);

    let mut conn = map_err_connection(self.pool.get_conn().await)?;

    map_err_query(conn.exec_drop(&sql, (id,)).await)?;

    Ok(true)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let mut sql = self.build_count_sql(collection);
    let mut params: Vec<JsonValue> = Vec::new();

    if let Some(f) = filter {
      let (clause, values) = self.query_builder.filter_to_sql_params(f)?;
      sql.push_str(" WHERE ");
      sql.push_str(&clause);
      params = values;
    }

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let values = convert_params(params);

    let result: Option<(i64,)> = if values.is_empty() {
      map_err_query(conn.exec_first(&sql, ()).await)?
    } else {
      map_err_query(conn.exec_first(&sql, &values).await)?
    };
    let (count,) = result.ok_or_else(|| OrmError::Query("No result".to_string()))?;

    Ok(count as u64)
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: JsonValue,
  ) -> OrmResult<usize> {
    let set_clauses = self.query_builder.build_set_clause(&updates, &[])?;

    let mut sql = self.build_update_sql(collection, &set_clauses);
    let mut params: Vec<JsonValue> = Vec::new();

    if let Some(f) = filter {
      let (clause, values) = self.query_builder.filter_to_sql_params(&f)?;
      sql.push_str(" WHERE ");
      sql.push_str(&clause);
      params = values;
    }

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let values = convert_params(params);

    if values.is_empty() {
      map_err_query(conn.exec_drop(&sql, ()).await)?;
    } else {
      map_err_query(conn.exec_drop(&sql, &values).await)?;
    }

    Ok(conn.affected_rows() as usize)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let mut sql = format!("DELETE FROM {}", self.dialect.quote_identifier(collection));
    let mut params: Vec<JsonValue> = Vec::new();

    if let Some(f) = filter {
      let (clause, values) = self.query_builder.filter_to_sql_params(&f)?;
      sql.push_str(" WHERE ");
      sql.push_str(&clause);
      params = values;
    }

    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let values = convert_params(params);

    if values.is_empty() {
      map_err_query(conn.exec_drop(&sql, ()).await)?;
    } else {
      map_err_query(conn.exec_drop(&sql, &values).await)?;
    }

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

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    self.list_mysql_indexes(collection).await
  }

  fn capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities::none()
      .with(Capability::Indexes)
      .with(Capability::RawQueries)
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

    let size: i64 = match conn.exec_iter(size_sql, (&collection,)).await {
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

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    self.list_mysql_indexes(collection).await
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    let sql = "SELECT DATABASE()";
    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let (name,): (String,) =
      map_err_query(conn.exec_first(sql, ()).await)?.unwrap_or(("unknown".to_string(),));
    Ok(name)
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    let sql = "SHOW DATABASES";
    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let rows: Vec<Row> = map_err_query(
      map_err_query(conn.exec_iter(sql, ()).await)?
        .collect()
        .await,
    )?;
    let names: Vec<String> = rows
      .iter()
      .map(|row| {
        let json = row::row_to_json_mysql(row.clone());
        json
          .get("Database")
          .or_else(|| json.get(0))
          .and_then(|v| v.as_str())
          .unwrap_or("")
          .to_string()
      })
      .collect();
    Ok(names)
  }
}

#[async_trait]
impl AdminCommands for MySqlProvider {
  async fn execute_raw(&self, query: &str, params: Vec<JsonValue>) -> OrmResult<RawResult> {
    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    let params_tuple = convert_params(params);

    let mut result = if params_tuple.is_empty() {
      map_err_query(conn.exec_iter(query, ()).await)?
    } else {
      map_err_query(conn.exec_iter(query, &params_tuple).await)?
    };

    let columns: Vec<String> = result
      .columns_ref()
      .iter()
      .map(|c| c.name_str().as_ref().to_string())
      .collect();

    if columns.is_empty() {
      let affected_rows = result.affected_rows();
      let last_insert_id = result.last_insert_id();
      map_err_query(result.drop_result().await)?;
      return Ok(RawResult {
        columns: vec![],
        rows: vec![],
        affected_rows,
        last_insert_id,
      });
    }

    let rows: Vec<Row> = map_err_query(result.collect().await)?;

    let rows_data: Vec<Vec<JsonValue>> = rows
      .iter()
      .map(|row: &Row| {
        let json = row::row_to_json_mysql(row.clone());
        columns
          .iter()
          .map(|name| json.get(name).cloned().unwrap_or(JsonValue::Null))
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
    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    map_err_query(conn.exec_drop(&sql, ()).await)?;
    Ok(())
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    let sql = self.build_drop_table_sql(collection);
    let mut conn = map_err_connection(self.pool.get_conn().await)?;
    map_err_query(conn.exec_drop(&sql, ()).await)?;
    Ok(())
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    let sql = self.build_rename_table_sql(from, to);
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

// MySQL operations each take a pool connection, so START TRANSACTION and
// COMMIT land on different connections. The pooled provider reports
// NotSupported instead of pretending otherwise.
#[async_trait]
impl TransactionControl for MySqlProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    Err(OrmError::NotSupported(
      "the MySQL provider uses a connection pool: a transaction needs one dedicated connection"
        .to_string(),
    ))
  }

  async fn commit_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the MySQL provider cannot commit a transaction".to_string(),
    ))
  }

  async fn rollback_transaction(&self, _id: TransactionId) -> OrmResult<()> {
    Err(OrmError::NotSupported(
      "the MySQL provider cannot roll back a transaction".to_string(),
    ))
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(false)
  }
}
