//! SQL query builder for generating SQL statements.

use crate::query::{Filter, OrderBy, Projection, SortDirection};
use crate::sql::types::{SqlDialect, SqlIndexDef, SqlTableDef};

/// SQL query builder for generating SQL statements from filters and projections.
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

  pub fn create_table_sql(&self, table: &SqlTableDef) -> String {
    table.to_sql(self.dialect)
  }

  pub fn drop_table_sql(&self, table_name: &str) -> String {
    let name = self.dialect.quote_identifier(table_name);
    format!("DROP TABLE {}", name)
  }

  pub fn create_index_sql(&self, table_name: &str, index: &SqlIndexDef) -> String {
    let unique_str = if index.unique { "UNIQUE " } else { "" };
    let columns = index
      .columns
      .iter()
      .map(|c| self.dialect.quote_identifier(c))
      .collect::<Vec<_>>()
      .join(", ");
    let name = self.dialect.quote_identifier(&index.name);
    format!(
      "CREATE {}INDEX {} ON {} ({})",
      unique_str,
      name,
      self.dialect.quote_identifier(table_name),
      columns
    )
  }

  pub fn build_create_index(&self, index: &SqlIndexDef) -> String {
    index.to_sql(self.dialect)
  }

  pub fn build_drop_index(&self, table_name: &str, index_name: &str) -> String {
    let index = self.dialect.quote_identifier(index_name);
    let table = self.dialect.quote_identifier(table_name);
    format!("DROP INDEX {} ON {}", index, table)
  }

  pub fn build_insert(&self, table: &str, columns: &[&str], values_count: usize) -> String {
    let table_name = self.dialect.quote_identifier(table);
    let cols = columns
      .iter()
      .map(|c| self.dialect.quote_identifier(c))
      .collect::<Vec<_>>()
      .join(", ");

    let placeholders = (0..values_count)
      .map(|i| self.dialect.parameter_placeholder(i))
      .collect::<Vec<_>>()
      .join(", ");

    format!(
      "INSERT INTO {} ({}) VALUES ({})",
      table_name, cols, placeholders
    )
  }

  pub fn build_create_table(&self, table: &SqlTableDef) -> String {
    table.to_sql(self.dialect)
  }

  pub fn build_drop_table(&self, table_name: &str, if_exists: bool) -> String {
    let name = self.dialect.quote_identifier(table_name);
    if if_exists {
      format!("DROP TABLE IF EXISTS {}", name)
    } else {
      format!("DROP TABLE {}", name)
    }
  }

  pub fn insert_sql(&self, table: &str, data: &serde_json::Value) -> String {
    let table_name = self.dialect.quote_identifier(table);
    let obj = data.as_object().expect("data must be an object");
    let columns: Vec<String> = obj
      .keys()
      .map(|k| self.dialect.quote_identifier(k))
      .collect();
    let placeholders: Vec<String> = obj.keys().map(|_| "?".to_string()).collect();
    format!(
      "INSERT INTO {} ({}) VALUES ({})",
      table_name,
      columns.join(", "),
      placeholders.join(", ")
    )
  }

  pub fn update_sql(
    &self,
    table: &str,
    data: &serde_json::Value,
    pk_field: &str,
    _pk_value: &str,
  ) -> String {
    let table_name = self.dialect.quote_identifier(table);
    let obj = data.as_object().expect("data must be an object");
    let set_clause = obj
      .keys()
      .map(|k| format!("{} = ?", self.dialect.quote_identifier(k)))
      .collect::<Vec<_>>()
      .join(", ");
    format!(
      "UPDATE {} SET {} WHERE {} = ?",
      table_name,
      set_clause,
      self.dialect.quote_identifier(pk_field)
    )
  }

  pub fn delete_sql(&self, table: &str, pk_field: &str, _pk_value: &str) -> String {
    let table_name = self.dialect.quote_identifier(table);
    format!(
      "DELETE FROM {} WHERE {} = ?",
      table_name,
      self.dialect.quote_identifier(pk_field)
    )
  }

  pub fn select_sql(
    &self,
    table: &str,
    projection: Option<&[String]>,
    limit: Option<u32>,
    offset: Option<u64>,
  ) -> String {
    let table_name = self.dialect.quote_identifier(table);
    let select_clause = projection
      .map(|p| {
        if p.is_empty() {
          "*".to_string()
        } else {
          p.iter()
            .map(|f| self.dialect.quote_identifier(f))
            .collect::<Vec<_>>()
            .join(", ")
        }
      })
      .unwrap_or_else(|| "*".to_string());

    let mut sql = format!("SELECT {} FROM {}", select_clause, table_name);
    sql.push_str(" ORDER BY id ASC");
    if let Some(l) = limit {
      sql.push_str(&format!(" LIMIT {}", l));
    }
    if let Some(o) = offset {
      sql.push_str(&format!(" OFFSET {}", o));
    }
    sql
  }

  pub fn build_select(
    &self,
    table: &str,
    filter: Option<&Filter>,
    projection: Option<&Projection>,
    order_by: Option<&[OrderBy]>,
    limit: Option<u32>,
    offset: Option<u64>,
  ) -> String {
    let table_name = self.dialect.quote_identifier(table);

    let select_clause = match projection {
      Some(p) => {
        if let Some(ref fields) = p.select {
          if fields.is_empty() {
            "*".to_string()
          } else {
            fields
              .iter()
              .map(|f| self.dialect.quote_identifier(f))
              .collect::<Vec<_>>()
              .join(", ")
          }
        } else if let Some(ref fields) = p.exclude {
          let all_cols = self.get_table_columns(table);
          all_cols
            .iter()
            .filter(|c| !fields.contains(c))
            .map(|c| self.dialect.quote_identifier(c))
            .collect::<Vec<_>>()
            .join(", ")
        } else {
          "*".to_string()
        }
      }
      None => "*".to_string(),
    };

    let mut sql = format!("SELECT {} FROM {}", select_clause, table_name);

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.filter_to_sql(f)));
    }

    if let Some(order) = order_by {
      if !order.is_empty() {
        let order_str = order
          .iter()
          .map(|o| {
            let dir = match o.direction {
              SortDirection::Asc => "ASC",
              SortDirection::Desc => "DESC",
            };
            format!("{} {}", self.dialect.quote_identifier(&o.field), dir)
          })
          .collect::<Vec<_>>()
          .join(", ");
        sql.push_str(&format!(" ORDER BY {}", order_str));
      }
    }

    if let Some(l) = limit {
      sql.push_str(&format!(" LIMIT {}", l));
    }

    if let Some(o) = offset {
      sql.push_str(&format!(" OFFSET {}", o));
    }

    sql
  }

  pub fn build_update(
    &self,
    table: &str,
    set_columns: &[(&str, String)],
    filter: Option<&Filter>,
  ) -> String {
    let table_name = self.dialect.quote_identifier(table);

    let set_clause = set_columns
      .iter()
      .map(|(col, _)| format!("{} = ?", self.dialect.quote_identifier(col)))
      .collect::<Vec<_>>()
      .join(", ");

    let mut sql = format!("UPDATE {} SET {}", table_name, set_clause);

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.filter_to_sql(f)));
    }

    sql
  }

  pub fn build_delete(&self, table: &str, filter: Option<&Filter>) -> String {
    let table_name = self.dialect.quote_identifier(table);

    let mut sql = format!("DELETE FROM {}", table_name);

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.filter_to_sql(f)));
    }

    sql
  }

  pub fn build_count(&self, table: &str, filter: Option<&Filter>) -> String {
    let table_name = self.dialect.quote_identifier(table);

    let mut sql = format!("SELECT COUNT(*) FROM {}", table_name);

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.filter_to_sql(f)));
    }

    sql
  }

  pub fn filter_to_sql(&self, filter: &Filter) -> String {
    match filter {
      Filter::Eq(field, value) => {
        format!(
          "{} = {}",
          self.dialect.quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::Ne(field, value) => {
        format!(
          "{} <> {}",
          self.dialect.quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::Gt(field, value) => {
        format!(
          "{} > {}",
          self.dialect.quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::Gte(field, value) => {
        format!(
          "{} >= {}",
          self.dialect.quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::Lt(field, value) => {
        format!(
          "{} < {}",
          self.dialect.quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::Lte(field, value) => {
        format!(
          "{} <= {}",
          self.dialect.quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::In(field, values) => {
        let values_str = values
          .iter()
          .map(|v| self.value_to_sql(v))
          .collect::<Vec<_>>()
          .join(", ");
        format!(
          "{} IN ({})",
          self.dialect.quote_identifier(field),
          values_str
        )
      }
      Filter::NotIn(field, values) => {
        let values_str = values
          .iter()
          .map(|v| self.value_to_sql(v))
          .collect::<Vec<_>>()
          .join(", ");
        format!(
          "{} NOT IN ({})",
          self.dialect.quote_identifier(field),
          values_str
        )
      }
      Filter::Contains(field, value) => {
        format!(
          "{} LIKE {}",
          self.dialect.quote_identifier(field),
          self.value_to_sql(&serde_json::json!(format!("%{}%", value)))
        )
      }
      Filter::StartsWith(field, prefix) => {
        format!(
          "{} LIKE {}",
          self.dialect.quote_identifier(field),
          self.value_to_sql(&serde_json::json!(format!("{}%", prefix)))
        )
      }
      Filter::IsNull(field) => {
        format!("{} IS NULL", self.dialect.quote_identifier(field))
      }
      Filter::IsNotNull(field) => {
        format!("{} IS NOT NULL", self.dialect.quote_identifier(field))
      }
      Filter::Like(field, pattern) => {
        format!(
          "{} LIKE {}",
          self.dialect.quote_identifier(field),
          self.value_to_sql(&serde_json::json!(pattern))
        )
      }
      Filter::EndsWith(field, suffix) => {
        format!(
          "{} LIKE {}",
          self.dialect.quote_identifier(field),
          self.value_to_sql(&serde_json::json!(format!("%{}", suffix)))
        )
      }
      Filter::Between(field, min, max) => {
        format!(
          "{} BETWEEN {} AND {}",
          self.dialect.quote_identifier(field),
          self.value_to_sql(min),
          self.value_to_sql(max)
        )
      }
      Filter::And(filters) => {
        let strs = filters
          .iter()
          .map(|f| format!("({})", self.filter_to_sql(f)))
          .collect::<Vec<_>>()
          .join(" AND ");
        strs
      }
      Filter::Or(filters) => {
        let strs = filters
          .iter()
          .map(|f| format!("({})", self.filter_to_sql(f)))
          .collect::<Vec<_>>()
          .join(" OR ");
        strs
      }
      Filter::Not(inner) => {
        format!("NOT ({})", self.filter_to_sql(inner))
      }
    }
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
        format!("'{}'", s.replace("'", "''"))
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

  fn get_table_columns(&self, _table: &str) -> Vec<String> {
    Vec::new()
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

  #[test]
  fn test_simple_filter() {
    let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);
    let filter = Filter::Eq("name".to_string(), serde_json::json!("Alice"));
    let sql = builder.filter_to_sql(&filter);
    assert_eq!(sql, "\"name\" = 'Alice'");
  }

  #[test]
  fn test_compound_filter() {
    let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);
    let filter = Filter::And(vec![
      Filter::Eq("age".to_string(), serde_json::json!(25)),
      Filter::Gt("balance".to_string(), serde_json::json!(100)),
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
