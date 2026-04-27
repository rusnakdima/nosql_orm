//! Shared row conversion utilities for SQL providers.

use crate::providers::sql::utils::base64_encode;
use serde_json::Value;

/// Convert a SQLite row to JSON value.
#[cfg(feature = "sql-sqlite")]
pub fn row_to_json_sqlite(row: &rusqlite::Row) -> Result<Value, rusqlite::Error> {
  use rusqlite::types::ValueRef;
  let mut map = serde_json::Map::new();

  for idx in 0..16 {
    let col_name = format!("col_{}", idx);
    let value = match row.get_ref(idx) {
      Ok(ValueRef::Null) => serde_json::Value::Null,
      Ok(ValueRef::Integer(i)) => serde_json::json!(i),
      Ok(ValueRef::Real(f)) => serde_json::json!(f),
      Ok(ValueRef::Text(s)) => {
        serde_json::json!(std::str::from_utf8(s).unwrap_or(""))
      }
      Ok(ValueRef::Blob(b)) => {
        serde_json::json!(base64_encode(&b))
      }
      Err(_) => break,
    };
    map.insert(col_name, value);
  }

  Ok(serde_json::Value::Object(map))
}

/// Convert a MySQL row to JSON value.
#[cfg(feature = "sql-mysql")]
pub fn row_to_json_mysql(row: mysql_async::Row) -> Value {
  use mysql_async::prelude::*;
  let mut map = serde_json::Map::new();
  let columns = row.columns();
  let len = columns.len();

  for i in 0..len {
    if let Some(col) = columns.get(i) {
      let col_name = col.name_str().as_ref().to_string();
      let col_value = match row.get::<i64, _>(i) {
        Some(v) => serde_json::json!(v),
        None => match row.get::<f64, _>(i) {
          Some(v) => serde_json::json!(v),
          None => match row.get::<String, _>(i) {
            Some(v) => serde_json::json!(v),
            None => match row.get::<Vec<u8>, _>(i) {
              Some(b) => {
                if let Ok(s) = std::str::from_utf8(&b) {
                  serde_json::Value::String(s.to_string())
                } else {
                  serde_json::Value::Null
                }
              }
              None => serde_json::Value::Null,
            },
          },
        },
      };
      map.insert(col_name, col_value);
    }
  }

  serde_json::Value::Object(map)
}

/// Convert a PostgreSQL row to JSON value.
#[cfg(feature = "sql-postgres")]
pub fn row_to_json_postgres(row: &tokio_postgres::Row) -> Value {
  use tokio_postgres::types::Type;
  let mut map = serde_json::Map::new();
  let columns = row.columns();
  for i in 0..columns.len() {
    let col = &columns[i];
    let col_name = col.name();
    let col_type = col.type_();
    let json_val = match *col_type {
      Type::BOOL => {
        serde_json::json!(row.get::<_, bool>(i))
      }
      Type::INT4 => {
        serde_json::json!(row.get::<_, i32>(i))
      }
      Type::INT8 => {
        serde_json::json!(row.get::<_, i64>(i))
      }
      Type::FLOAT4 => {
        serde_json::json!(row.get::<_, f32>(i))
      }
      Type::FLOAT8 => {
        serde_json::json!(row.get::<_, f64>(i))
      }
      Type::TEXT | Type::VARCHAR => {
        serde_json::json!(row.get::<_, String>(i))
      }
      Type::JSON | Type::JSONB => {
        serde_json::from_str(&row.get::<_, String>(i)).unwrap_or(serde_json::Value::Null)
      }
      Type::BYTEA => {
        serde_json::json!(base64_encode(&row.get::<_, Vec<u8>>(i)))
      }
      _ => serde_json::Value::Null,
    };
    map.insert(col_name.to_string(), json_val);
  }
  serde_json::Value::Object(map)
}
