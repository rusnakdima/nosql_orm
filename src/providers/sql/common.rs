use crate::error::{OrmError, OrmResult};
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use serde_json::Value;

pub trait SqlProviderHelpers {
  fn dialect(&self) -> SqlDialect;
  fn query_builder(&self) -> &SqlQueryBuilder;

  fn build_create_table_sql(&self, collection: &str) -> String {
    format!(
      "CREATE TABLE {} (id TEXT PRIMARY KEY)",
      self.dialect().quote_identifier(collection)
    )
  }

  fn build_drop_table_sql(&self, collection: &str) -> String {
    format!(
      "DROP TABLE IF EXISTS {}",
      self.dialect().quote_identifier(collection)
    )
  }

  fn map_sql_error(&self, e: rusqlite::Error) -> OrmError {
    OrmError::Query(format!("SQLite error: {}", e))
  }

  fn value_to_json(&self, value: &Value) -> Value {
    value.clone()
  }
}

pub fn build_insert_sql(
  dialect: &SqlDialect,
  collection: &str,
  columns: &[&str],
  values: &[String],
) -> String {
  let columns_str = columns
    .iter()
    .map(|c| dialect.quote_identifier(c))
    .collect::<Vec<_>>()
    .join(", ");

  format!(
    "INSERT INTO {} ({}) VALUES ({})",
    dialect.quote_identifier(collection),
    columns_str,
    values.join(", ")
  )
}

pub fn build_select_by_id_sql(dialect: &SqlDialect, collection: &str) -> String {
  format!(
    "SELECT * FROM {} WHERE id = ?",
    dialect.quote_identifier(collection)
  )
}

pub fn build_delete_sql(dialect: &SqlDialect, collection: &str) -> String {
  format!(
    "DELETE FROM {} WHERE id = ?",
    dialect.quote_identifier(collection)
  )
}
