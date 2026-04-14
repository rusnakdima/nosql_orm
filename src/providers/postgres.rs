use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use async_trait::async_trait;
use serde_json::Value;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Column, Row};

#[derive(Clone)]
pub struct PostgresProvider {
  pool: PgPool,
}

impl PostgresProvider {
  pub async fn new(connection_string: &str) -> OrmResult<Self> {
    let pool = PgPoolOptions::new()
      .max_connections(10)
      .connect(connection_string)
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;
    Ok(Self { pool })
  }

  pub async fn with_pool(pool: PgPool) -> Self {
    Self { pool }
  }

  pub fn pool(&self) -> &PgPool {
    &self.pool
  }

  fn value_to_pg_param(value: &Value) -> String {
    match value {
      Value::Null => "NULL".to_string(),
      Value::Bool(b) => b.to_string(),
      Value::Number(n) => n.to_string(),
      Value::String(s) => format!("'{}'", s.replace('\'', "''")),
      Value::Array(arr) => format!("'{}'", serde_json::to_string(arr).unwrap_or_default()),
      Value::Object(obj) => format!("'{}'", serde_json::to_string(obj).unwrap_or_default()),
    }
  }

  fn filter_to_where(filter: &Filter) -> (String, Vec<String>) {
    let mut conditions = Vec::new();
    let mut params = Vec::new();

    match filter {
      Filter::Eq(field, value) => {
        conditions.push(format!("{} = ${}", field, params.len() + 1));
        params.push(Self::value_to_pg_param(value));
      }
      Filter::Ne(field, value) => {
        conditions.push(format!("{} <> ${}", field, params.len() + 1));
        params.push(Self::value_to_pg_param(value));
      }
      Filter::Gt(field, value) => {
        conditions.push(format!("{} > ${}", field, params.len() + 1));
        params.push(Self::value_to_pg_param(value));
      }
      Filter::Lt(field, value) => {
        conditions.push(format!("{} < ${}", field, params.len() + 1));
        params.push(Self::value_to_pg_param(value));
      }
      Filter::Gte(field, value) => {
        conditions.push(format!("{} >= ${}", field, params.len() + 1));
        params.push(Self::value_to_pg_param(value));
      }
      Filter::Lte(field, value) => {
        conditions.push(format!("{} <= ${}", field, params.len() + 1));
        params.push(Self::value_to_pg_param(value));
      }
      Filter::In(field, values) => {
        let placeholders: Vec<String> = values
          .iter()
          .enumerate()
          .map(|(i, v)| format!("${}", params.len() + i + 1))
          .collect();
        conditions.push(format!("{} IN ({})", field, placeholders.join(", ")));
        for v in values {
          params.push(Self::value_to_pg_param(v));
        }
      }
      Filter::NotIn(field, values) => {
        let placeholders: Vec<String> = values
          .iter()
          .enumerate()
          .map(|(i, v)| format!("${}", params.len() + i + 1))
          .collect();
        conditions.push(format!("{} NOT IN ({})", field, placeholders.join(", ")));
        for v in values {
          params.push(Self::value_to_pg_param(v));
        }
      }
      Filter::Contains(field, sub) => {
        conditions.push(format!("{} ILIKE ${}", field, params.len() + 1));
        params.push(format!("%{}%", sub));
      }
      Filter::StartsWith(field, prefix) => {
        conditions.push(format!("{} ILIKE ${}", field, params.len() + 1));
        params.push(format!("{}%", prefix));
      }
      Filter::And(filters) => {
        let (parts, ps) = Self::filters_to_where(filters);
        conditions.push(format!("({})", parts.join(" AND ")));
        params.extend(ps);
      }
      Filter::Or(filters) => {
        let (parts, ps) = Self::filters_to_where(filters);
        conditions.push(format!("({})", parts.join(" OR ")));
        params.extend(ps);
      }
      Filter::Not(inner) => {
        let (cond, ps) = Self::filter_to_where(inner);
        conditions.push(format!("NOT ({})", cond));
        params.extend(ps);
      }
      Filter::IsNull(field) => {
        conditions.push(format!("{} IS NULL", field));
      }
    }

    (conditions.join(" AND "), params)
  }

  fn filters_to_where(filters: &[Filter]) -> (Vec<String>, Vec<String>) {
    let mut conditions = Vec::new();
    let mut params = Vec::new();
    for f in filters {
      let (cond, ps) = Self::filter_to_where(f);
      conditions.push(cond);
      params.extend(ps);
    }
    (conditions, params)
  }
}

#[async_trait]
impl DatabaseProvider for PostgresProvider {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut doc_with_id = doc.clone();
    if let Some(obj) = doc_with_id.as_object_mut() {
      obj.insert("id".to_string(), Value::String(id.clone()));
    }

    let columns: Vec<String> = doc_with_id
      .as_object()
      .map(|m| m.keys().map(|k| k.to_string()).collect())
      .unwrap_or_default();
    let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${}", i)).collect();

    let sql = format!(
      "INSERT INTO {} ({}) VALUES ({}) RETURNING *",
      collection,
      columns.join(", "),
      placeholders.join(", ")
    );

    let mut query = sqlx::query(&sql);
    if let Some(obj) = doc_with_id.as_object() {
      for (_, v) in obj {
        query = query.bind(Self::value_to_pg_param(v));
      }
    }

    let row = query
      .fetch_one(&self.pool)
      .await
      .map_err(|e| OrmError::Provider(e.to_string()))?;

    let mut map = serde_json::Map::new();
    for (i, col) in columns.iter().enumerate() {
      if let Ok(val) = row.try_get::<String, _>(i) {
        map.insert(
          col.clone(),
          serde_json::from_str(&val).unwrap_or(Value::String(val)),
        );
      }
    }
    Ok(Value::Object(map))
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let sql = format!("SELECT * FROM {} WHERE id = $1", collection);
    let row = sqlx::query(&sql)
      .bind(id)
      .fetch_optional(&self.pool)
      .await
      .map_err(|e| OrmError::Provider(e.to_string()))?;

    Ok(row.map(|r| {
      let mut map = serde_json::Map::new();
      for col in r.columns() {
        if let Ok(val) = r.try_get::<String, _>(col.name()) {
          map.insert(
            col.name().to_string(),
            serde_json::from_str(&val).unwrap_or(Value::String(val)),
          );
        }
      }
      Value::Object(map)
    }))
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
    let mut sql = format!("SELECT * FROM {}", collection);
    let mut params: Vec<String> = Vec::new();

    if let Some(f) = filter {
      let (where_clause, ps) = Self::filter_to_where(f);
      sql.push_str(&format!(" WHERE {}", where_clause));
      params.extend(ps);
    }

    if let Some(field) = sort_by {
      let dir = if sort_asc { "ASC" } else { "DESC" };
      sql.push_str(&format!(" ORDER BY {} {}", field, dir));
    }

    if let Some(s) = skip {
      sql.push_str(&format!(" OFFSET {}", s));
    }

    if let Some(l) = limit {
      sql.push_str(&format!(" LIMIT {}", l));
    }

    let query = sqlx::query(&sql);
    let rows = query
      .fetch_all(&self.pool)
      .await
      .map_err(|e| OrmError::Provider(e.to_string()))?;

    Ok(
      rows
        .iter()
        .map(|r| {
          let mut map = serde_json::Map::new();
          for col in r.columns() {
            if let Ok(val) = r.try_get::<String, _>(col.name()) {
              map.insert(
                col.name().to_string(),
                serde_json::from_str(&val).unwrap_or(Value::String(val)),
              );
            }
          }
          Value::Object(map)
        })
        .collect(),
    )
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let mut sets = Vec::new();
    let mut param_idx = 1;

    if let Some(obj) = doc.as_object() {
      for (key, value) in obj {
        if key != "id" {
          sets.push(format!("{} = ${}", key, param_idx));
          param_idx += 1;
        }
      }
    }

    if sets.is_empty() {
      return self
        .find_by_id(collection, id)
        .await?
        .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)));
    }

    let sql = format!(
      "UPDATE {} SET {} WHERE id = ${} RETURNING *",
      collection,
      sets.join(", "),
      param_idx
    );

    let mut query = sqlx::query(&sql);
    if let Some(obj) = doc.as_object() {
      for (key, value) in obj {
        if key != "id" {
          query = query.bind(Self::value_to_pg_param(value));
        }
      }
    }
    query = query.bind(id);

    let row = query
      .fetch_one(&self.pool)
      .await
      .map_err(|e| OrmError::Provider(e.to_string()))?;

    let mut map = serde_json::Map::new();
    for col in row.columns() {
      if let Ok(val) = row.try_get::<String, _>(col.name()) {
        map.insert(
          col.name().to_string(),
          serde_json::from_str(&val).unwrap_or(Value::String(val)),
        );
      }
    }
    Ok(Value::Object(map))
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let mut sets = Vec::new();
    let mut param_idx = 1;

    if let Some(obj) = patch.as_object() {
      for (key, value) in obj {
        sets.push(format!("{} = ${}", key, param_idx));
        param_idx += 1;
      }
    }

    let sql = format!(
      "UPDATE {} SET {} WHERE id = ${} RETURNING *",
      collection,
      sets.join(", "),
      param_idx
    );

    let mut query = sqlx::query(&sql);
    if let Some(obj) = patch.as_object() {
      for (_, value) in obj {
        query = query.bind(Self::value_to_pg_param(value));
      }
    }
    query = query.bind(id);

    let row = query
      .fetch_one(&self.pool)
      .await
      .map_err(|e| OrmError::Provider(e.to_string()))?;

    let mut map = serde_json::Map::new();
    for col in row.columns() {
      if let Ok(val) = row.try_get::<String, _>(col.name()) {
        map.insert(
          col.name().to_string(),
          serde_json::from_str(&val).unwrap_or(Value::String(val)),
        );
      }
    }
    Ok(Value::Object(map))
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let sql = format!("DELETE FROM {} WHERE id = $1", collection);
    let result = sqlx::query(&sql)
      .bind(id)
      .execute(&self.pool)
      .await
      .map_err(|e| OrmError::Provider(e.to_string()))?;
    Ok(result.rows_affected() > 0)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let mut sql = format!("SELECT COUNT(*) FROM {}", collection);

    if let Some(f) = filter {
      let (where_clause, _) = Self::filter_to_where(f);
      sql.push_str(&format!(" WHERE {}", where_clause));
    }

    let row = sqlx::query(&sql)
      .fetch_one(&self.pool)
      .await
      .map_err(|e| OrmError::Provider(e.to_string()))?;

    Ok(row.get::<i64, _>(0) as u64)
  }
}
