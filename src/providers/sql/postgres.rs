//! PostgreSQL provider for nosql_orm.

use crate::error::{OrmError, OrmResult};
use crate::nosql_index::{NosqlIndex, NosqlIndexInfo};
use crate::provider::{DatabaseProvider, ProviderConfig};
use crate::query::Filter;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use async_trait::async_trait;
use deadpool_postgres::{Config, Manager, ManagerConfig, Pool, RecyclingMethod};
use serde_json::Value;
use std::collections::HashMap;
use tokio_postgres::Row;

/// PostgreSQL-backed provider.
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

  fn row_to_json(row: &Row) -> Value {
    let mut map = serde_json::Map::new();
    let columns = row.columns();
    for i in 0..columns.len() {
      let col = &columns[i];
      let col_name = col.name();
      let col_type = col.type_();
      let json_val = match *col_type {
        tokio_postgres::types::Type::BOOL => {
          serde_json::json!(row.get::<_, bool>(i))
        }
        tokio_postgres::types::Type::INT4 => {
          serde_json::json!(row.get::<_, i32>(i))
        }
        tokio_postgres::types::Type::INT8 => {
          serde_json::json!(row.get::<_, i64>(i))
        }
        tokio_postgres::types::Type::FLOAT4 => {
          serde_json::json!(row.get::<_, f32>(i))
        }
        tokio_postgres::types::Type::FLOAT8 => {
          serde_json::json!(row.get::<_, f64>(i))
        }
        tokio_postgres::types::Type::TEXT | tokio_postgres::types::Type::VARCHAR => {
          serde_json::json!(row.get::<_, String>(i))
        }
        tokio_postgres::types::Type::JSON | tokio_postgres::types::Type::JSONB => {
          serde_json::from_str(&row.get::<_, String>(i)).unwrap_or(serde_json::Value::Null)
        }
        tokio_postgres::types::Type::BYTEA => {
          serde_json::json!(base64_encode(&row.get::<_, Vec<u8>>(i)))
        }
        _ => serde_json::Value::Null,
      };
      map.insert(col_name.to_string(), json_val);
    }
    serde_json::Value::Object(map)
  }

  pub fn dialect(&self) -> SqlDialect {
    self.dialect
  }
}

fn base64_encode(data: &[u8]) -> String {
  use base64::{engine::general_purpose, Engine as _};
  general_purpose::STANDARD.encode(data)
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

    let client = self
      .pool
      .get()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;

    let row = client
      .query_one(&sql, &[])
      .await
      .map_err(|e| OrmError::Query(e.to_string()))?;

    Ok(Self::row_to_json(&row))
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let sql = format!(
      "SELECT * FROM {} WHERE id = $1",
      self.dialect.quote_identifier(collection)
    );

    let client = self
      .pool
      .get()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;

    let row = client
      .query_opt(&sql, &[&id])
      .await
      .map_err(|e| OrmError::Query(e.to_string()))?;

    Ok(row.map(|r| Self::row_to_json(&r)))
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

    let mut params: Vec<String> = Vec::new();
    let mut param_idx = 0;

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

    let client = self
      .pool
      .get()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;

    let rows = client
      .query(&sql, &[])
      .await
      .map_err(|e| OrmError::Query(e.to_string()))?;

    Ok(rows.iter().map(|r| Self::row_to_json(r)).collect())
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let doc_obj = doc
      .as_object()
      .ok_or_else(|| OrmError::InvalidInput("Document must be an object".to_string()))?;

    let set_clauses: Vec<String> = doc_obj
      .iter()
      .filter(|(k, _)| *k != "id")
      .map(|(k, v)| {
        format!(
          "{} = {}",
          self.dialect.quote_identifier(k),
          self.query_builder.value_to_sql(v)
        )
      })
      .collect();

    let sql = format!(
      "UPDATE {} SET {} WHERE id = $1 RETURNING *",
      self.dialect.quote_identifier(collection),
      set_clauses.join(", ")
    );

    let client = self
      .pool
      .get()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;

    let row = client
      .query_one(&sql, &[&id])
      .await
      .map_err(|e| OrmError::Query(e.to_string()))?;

    Ok(Self::row_to_json(&row))
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let patch_obj = patch
      .as_object()
      .ok_or_else(|| OrmError::InvalidInput("Patch must be an object".to_string()))?;

    let set_clauses: Vec<String> = patch_obj
      .iter()
      .map(|(k, v)| {
        format!(
          "{} = {}",
          self.dialect.quote_identifier(k),
          self.query_builder.value_to_sql(v)
        )
      })
      .collect();

    let sql = format!(
      "UPDATE {} SET {} WHERE id = $1 RETURNING *",
      self.dialect.quote_identifier(collection),
      set_clauses.join(", ")
    );

    let client = self
      .pool
      .get()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;

    let row = client
      .query_one(&sql, &[&id])
      .await
      .map_err(|e| OrmError::Query(e.to_string()))?;

    Ok(Self::row_to_json(&row))
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let sql = format!(
      "DELETE FROM {} WHERE id = $1",
      self.dialect.quote_identifier(collection)
    );

    let client = self
      .pool
      .get()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;

    let result = client
      .execute(&sql, &[&id])
      .await
      .map_err(|e| OrmError::Query(e.to_string()))?;

    Ok(result > 0)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let mut sql = format!(
      "SELECT COUNT(*) FROM {}",
      self.dialect.quote_identifier(collection)
    );

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.query_builder.filter_to_sql(f)));
    }

    let client = self
      .pool
      .get()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;

    let row = client
      .query_one(&sql, &[])
      .await
      .map_err(|e| OrmError::Query(e.to_string()))?;

    let count: i64 = row.get(0);
    Ok(count as u64)
  }

  async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()> {
    use crate::nosql_index::NosqlIndexType;

    let mut index_def = crate::sql::types::SqlIndexDef::new(
      index.get_name().unwrap_or("idx_default"),
      collection,
      index.get_fields().iter().map(|(f, _)| f.clone()).collect(),
    );

    if index.is_unique() {
      index_def = index_def.unique();
    }

    let sql = self.query_builder.build_create_index(&index_def);

    let client = self
      .pool
      .get()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;

    client
      .execute(&sql, &[])
      .await
      .map_err(|e| OrmError::Query(e.to_string()))?;

    Ok(())
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    let sql = format!(
      "DROP INDEX IF EXISTS {}",
      self.dialect.quote_identifier(index_name)
    );

    let client = self
      .pool
      .get()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;

    client
      .execute(&sql, &[])
      .await
      .map_err(|e| OrmError::Query(e.to_string()))?;

    Ok(())
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<NosqlIndexInfo>> {
    let sql = "
            SELECT indexname, indexdef
            FROM pg_indexes
            WHERE schemaname = 'public' AND tablename = $1
        ";

    let client = self
      .pool
      .get()
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;

    let rows = client
      .query(sql, &[&collection])
      .await
      .map_err(|e| OrmError::Query(e.to_string()))?;

    let indexes = rows
      .iter()
      .map(|row| {
        let name: String = row.get("indexname");
        let indexdef: String = row.get("indexdef");
        let unique = indexdef.contains("UNIQUE");
        let fields = extract_index_fields(&indexdef);

        NosqlIndexInfo {
          name,
          namespace: format!("public.{}", collection),
          unique,
          sparse: false,
          ttl_seconds: None,
          version: None,
          index_type: determine_index_type(&indexdef),
          fields,
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
