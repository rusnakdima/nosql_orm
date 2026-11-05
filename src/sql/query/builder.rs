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
        format!("'{}'", self.escape_string_literal(s))
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

  // Writes values that are not bound parameters (INSERT/SET clauses). MySQL
  // treats a backslash inside a string literal as an escape, so it must be
  // doubled there; PostgreSQL and SQLite use standard-conforming strings,
  // where a backslash is literal and only the quote needs doubling. The bound
  // filter path (filter_to_sql_params) is the remaining place free of literal
  // interpolation; INSERT/SET should move to bound columns next.
  pub fn escape_string_literal(&self, value: &str) -> String {
    match self.dialect {
      SqlDialect::MySQL => {
        let mut escaped = String::with_capacity(value.len());
        for ch in value.chars() {
          match ch {
            '\'' => escaped.push_str("''"),
            '\\' => escaped.push_str("\\\\"),
            '\0' => escaped.push_str("\\0"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\x1a' => escaped.push_str("\\Z"),
            _ => escaped.push(ch),
          }
        }
        escaped
      }
      // A NUL byte has no representation in a PostgreSQL or SQLite text
      // literal; it is left in place so the statement is rejected loudly
      // instead of silently storing a different value.
      SqlDialect::PostgreSQL | SqlDialect::SQLite => value.replace('\'', "''"),
    }
  }
}

impl Default for SqlQueryBuilder {
  fn default() -> Self {
    Self::new(SqlDialect::PostgreSQL)
  }
}

#[cfg(test)]
mod tests;
