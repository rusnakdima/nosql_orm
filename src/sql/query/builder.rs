//! SQL query builder for generating SQL statements.

use crate::error::{OrmError, OrmResult};
use crate::sql::types::SqlDialect;

#[derive(Clone)]
pub struct SqlQueryBuilder {
  dialect: SqlDialect,
}

impl SqlQueryBuilder {
  pub fn new(dialect: SqlDialect) -> Self {
    Self { dialect }
  }

  pub fn dialect(&self) -> SqlDialect {
    self.dialect
  }

  pub(crate) fn get_table_columns(&self, _table: &str) -> Vec<String> {
    Vec::new()
  }

  pub fn build_set_clause(
    &self,
    doc: &serde_json::Value,
    exclude_fields: &[&str],
  ) -> OrmResult<Vec<String>> {
    let obj = doc
      .as_object()
      .ok_or_else(|| OrmError::Internal("Document must be an object".to_string()))?;

    let clauses: Vec<String> = obj
      .iter()
      .filter(|(k, _)| !exclude_fields.contains(&k.as_str()))
      .map(|(k, v)| {
        Ok(format!(
          "{} = {}",
          self.dialect.quote_identifier(k),
          self.value_to_sql(v)
        ))
      })
      .collect::<OrmResult<Vec<String>>>()?;

    Ok(clauses)
  }

  pub fn value_to_sql(&self, value: &serde_json::Value) -> String {
    match value {
      serde_json::Value::Null => "NULL".to_string(),
      serde_json::Value::Bool(b) => {
        if *b {
          "TRUE".to_string()
        } else {
          "FALSE".to_string()
        }
      }
      serde_json::Value::Number(n) => n.to_string(),
      serde_json::Value::String(s) => {
        format!("'{}'", s.replace('\'', "''"))
      }
      serde_json::Value::Array(arr) => {
        let items = arr
          .iter()
          .map(|v| self.value_to_sql(v))
          .collect::<Vec<_>>()
          .join(", ");
        format!("({})", items)
      }
      serde_json::Value::Object(obj) => {
        let pairs = obj
          .iter()
          .map(|(k, v)| format!("{}: {}", k, self.value_to_sql(v)))
          .collect::<Vec<_>>()
          .join(", ");
        format!("'{{{}}}'", pairs)
      }
    }
  }
}

impl Default for SqlQueryBuilder {
  fn default() -> Self {
    Self::new(SqlDialect::PostgreSQL)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::query::Projection;
  use crate::sql::types::SqlDialect;

  #[test]
  fn test_simple_filter() {
    let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);
    let filter = crate::query::Filter::Eq("name".to_string(), serde_json::json!("Alice"));
    let sql = builder.filter_to_sql(&filter);
    assert_eq!(sql, "\"name\" = 'Alice'");
  }

  #[test]
  fn test_compound_filter() {
    let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);
    let filter = crate::query::Filter::And(vec![
      crate::query::Filter::Eq("age".to_string(), serde_json::json!(25)),
      crate::query::Filter::Gt("balance".to_string(), serde_json::json!(100)),
    ]);
    let sql = builder.filter_to_sql(&filter);
    assert!(sql.contains("\"age\" = 25"));
    assert!(sql.contains("\"balance\" > 100"));
  }

  #[test]
  fn test_insert_sql() {
    let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);
    let sql = builder.build_insert("users", &["name", "email"], 2);
    assert_eq!(
      sql,
      "INSERT INTO \"users\" (\"name\", \"email\") VALUES ($1, $2)"
    );
  }

  #[test]
  fn test_select_with_projection() {
    let builder = SqlQueryBuilder::new(SqlDialect::MySQL);
    let projection = Projection {
      select: Some(vec!["id".to_string(), "name".to_string()]),
      exclude: None,
    };
    let sql = builder.build_select("users", None, Some(&projection), None, None, None);
    assert_eq!(sql, "SELECT `id`, `name` FROM `users`");
  }

  #[test]
  fn test_mysql_quote() {
    let builder = SqlQueryBuilder::new(SqlDialect::MySQL);
    assert_eq!(builder.dialect().quote_identifier("name"), "`name`");
  }
}
